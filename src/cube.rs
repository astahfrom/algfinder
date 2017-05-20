extern crate rayon;

use self::rayon::prelude::*;

use std::fmt;
use std::sync::mpsc::Sender;

/*
Cube layout

        012
        345
        678

876 012 012 012
543 345 345 345
210 678 678 678

        012
        345
        678
 */

const SHIFT2: u32 = 6;
const SHIFT4: u32 = 12;
const SHIFT6: u32 = 18;
const SHIFT8: u32 = 24;

const PIECE0: u32 = 0b000_000_000_000_000_000_000_000_111;
const PIECE1: u32 = 0b000_000_000_000_000_000_000_111_000;
const PIECE2: u32 = 0b000_000_000_000_000_000_111_000_000;
const PIECE3: u32 = 0b000_000_000_000_000_111_000_000_000;
const PIECE4: u32 = 0b000_000_000_000_111_000_000_000_000;
const PIECE5: u32 = 0b000_000_000_111_000_000_000_000_000;
const PIECE6: u32 = 0b000_000_111_000_000_000_000_000_000;
const PIECE7: u32 = 0b000_111_000_000_000_000_000_000_000;
const PIECE8: u32 = 0b111_000_000_000_000_000_000_000_000;

const MASK012: u32 = PIECE0 | PIECE1 | PIECE2;
const MASK036: u32 = PIECE0 | PIECE3 | PIECE6;
const MASK147: u32 = PIECE1 | PIECE4 | PIECE7;
const MASK258: u32 = PIECE2 | PIECE5 | PIECE8;
const MASK678: u32 = PIECE6 | PIECE7 | PIECE8;

#[derive(Clone, Copy, Debug)]
pub struct Cube<T = u32> {
    pub up: T,
    pub down: T,
    pub left: T,
    pub right: T,
    pub front: T,
    pub back: T,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Color {
    Grey = 0,
    White = 1,
    Yellow = 2,
    Green = 3,
    Blue = 4,
    Red = 5,
    Orange = 6,
}

#[derive(Debug, Clone, Copy)]
pub enum Turn {
    U = 0b0,
    U_ = 0b1,
    U2 = 0b10,
    D = 0b100,
    D_ = 0b101,
    D2 = 0b110,
    L = 0b1000,
    L_ = 0b1001,
    L2 = 0b1010,
    R = 0b10000,
    R_ = 0b10001,
    R2 = 0b10010,
    F = 0b100000,
    F_ = 0b100001,
    F2 = 0b100010,
    B = 0b1000000,
    B_ = 0b1000001,
    B2 = 0b1000010,
    M = 0b10000000,
    M_ = 0b10000001,
    M2 = 0b10000010,
}

pub type Algorithm = Vec<Turn>;

#[derive(Debug, Clone)]
pub enum SearchResult {
    Algorithm(Algorithm),
    Depth(usize),
}

impl fmt::Display for Turn {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use self::Turn::*;

        let s = match *self {
            U => "U",
            U_ => "U'",
            U2 => "U2",
            D => "D",
            D_ => "D'",
            D2 => "D2",
            L => "L",
            L_ => "L'",
            L2 => "L2",
            R => "R",
            R_ => "R'",
            R2 => "R2",
            F => "F",
            F_ => "F'",
            F2 => "F2",
            B => "B",
            B_ => "B'",
            B2 => "B2",
            M => "M",
            M_ => "M'",
            M2 => "M2",
        };

        write!(f, "{}", s)
    }
}

impl fmt::Display for Color {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use self::Color::*;

        let c = match *self {
            Grey => '_',
            White => 'W',
            Yellow => 'Y',
            Green => 'G',
            Blue => 'B',
            Red => 'R',
            Orange => 'O',
        };

        write!(f, "{}", c)
    }
}

impl<'a> Cube<Vec<Color>> {
    fn face_from_colors(colors: &[Color]) -> u32 {
        let mut face = 0;

        for (i, &color) in colors.iter().enumerate() {
            face |= (color as u32) << (3 * i);
        }

        face
    }

    // Assumes the colors are layouted correctly
    pub fn pack(&self) -> Cube {
        Cube {
            up: Self::face_from_colors(&self.up),
            down: Self::face_from_colors(&self.down),
            left: Self::face_from_colors(&self.left),
            right: Self::face_from_colors(&self.right),
            front: Self::face_from_colors(&self.front),
            back: Self::face_from_colors(&self.back),
        }
    }
}

impl Cube {
    // Yellow on top, green in front
    pub fn solved_state() -> Self {
        Cube {
            up: 0b010010010010010010010010010,
            down: 0b001001001001001001001001001,
            left: 0b101101101101101101101101101,
            right: 0b110110110110110110110110110,
            front: 0b011011011011011011011011011,
            back: 0b100100100100100100100100100,
        }
    }

    fn faces(&self) -> [u32; 6] {
        [self.up, self.down, self.left, self.right, self.front, self.back]
    }

    fn colors_in_face(face: u32) -> ([u8; 6], [u8; 6]) {
        let (mut corners, mut edges) = ([0; 6], [0; 6]);

        for &corner in &[0, 2, 6, 8] {
            let col = (face >> (3 * corner)) & 0b111;

            if col > 0 {
                corners[col as usize - 1] += 1;
            }
        }

        for &edge in &[1, 3, 4, 5, 7] {
            let col = (face >> (3 * edge)) & 0b111;

            if col > 0 {
                edges[col as usize - 1] += 1;
            }
        }

        (corners, edges)
    }

    fn colors(&self) -> ([u8; 6], [u8; 6]) {
        let add = |xss: [u8; 6], xs: [u8; 6]| {
            [xss[0] + xs[0],
             xss[1] + xs[1],
             xss[2] + xs[2],
             xss[3] + xs[3],
             xss[4] + xs[4],
             xss[5] + xs[5]]
        };

        self.faces()
            .iter()
            .map(|&face| Self::colors_in_face(face))
            .fold(([0; 6], [0; 6]),
                  |(css, ess), (cs, es)| (add(css, cs), add(ess, es)))
    }

    pub fn missing_colors(&self, pattern: &Cube) -> Vec<Color> {
        use Color::*;

        let mut missing = Vec::new();

        let colors = [White, Yellow, Green, Blue, Red, Orange];

        let (from_corners, from_edges) = self.colors();
        let (to_corners, to_edges) = pattern.colors();

        for &color in &colors {
            let i = color as usize - 1;

            if from_corners[i] < to_corners[i] || from_edges[i] < to_edges[i] {
                missing.push(color);
            }
        }

        missing
    }

    fn matches_face(face: u32, pattern: u32) -> bool {
        let grey = Color::Grey as u32;

        ((pattern & PIECE0) == grey || (pattern & PIECE0 == face & PIECE0)) &&
        ((pattern & PIECE1) == grey || (pattern & PIECE1 == face & PIECE1)) &&
        ((pattern & PIECE2) == grey || (pattern & PIECE2 == face & PIECE2)) &&
        ((pattern & PIECE3) == grey || (pattern & PIECE3 == face & PIECE3)) &&
        ((pattern & PIECE4) == grey || (pattern & PIECE4 == face & PIECE4)) &&
        ((pattern & PIECE5) == grey || (pattern & PIECE5 == face & PIECE5)) &&
        ((pattern & PIECE6) == grey || (pattern & PIECE6 == face & PIECE6)) &&
        ((pattern & PIECE7) == grey || (pattern & PIECE7 == face & PIECE7)) &&
        ((pattern & PIECE8) == grey || (pattern & PIECE8 == face & PIECE8))
    }

    fn matches(&self, other: &Cube) -> bool {
        ((self.up & other.up) == other.up) && ((self.down & other.down) == other.down) &&
        ((self.left & other.left) == other.left) &&
        ((self.right & other.right) == other.right) &&
        ((self.front & other.front) == other.front) &&
        ((self.back & other.back) == other.back) && Self::matches_face(self.up, other.up) &&
        Self::matches_face(self.down, other.down) &&
        Self::matches_face(self.left, other.left) &&
        Self::matches_face(self.right, other.right) &&
        Self::matches_face(self.front, other.front) &&
        Self::matches_face(self.back, other.back)
    }

    fn rotate_face(face: u32) -> u32 {
        let part4 = face & PIECE4;

        let part05 = (face & (PIECE0 | PIECE5)) << SHIFT2;
        let part1 = (face & PIECE1) << SHIFT4;
        let part2 = (face & PIECE2) << SHIFT6;

        let part38 = (face & (PIECE3 | PIECE8)) >> SHIFT2;
        let part7 = (face & PIECE7) >> SHIFT4;
        let part6 = (face & PIECE6) >> SHIFT6;

        part4 | part05 | part1 | part2 | part38 | part7 | part6
    }

    fn rotate_face_(face: u32) -> u32 {
        let part4 = face & PIECE4;

        let part16 = (face & (PIECE1 | PIECE6)) << SHIFT2;
        let part3 = (face & PIECE3) << SHIFT4;
        let part0 = (face & PIECE0) << SHIFT6;

        let part27 = (face & (PIECE2 | PIECE7)) >> SHIFT2;
        let part5 = (face & PIECE5) >> SHIFT4;
        let part8 = (face & PIECE8) >> SHIFT6;

        part4 | part16 | part3 | part0 | part27 | part5 | part8
    }

    fn rotate_face2(face: u32) -> u32 {
        ((face & PIECE0) << SHIFT8) | ((face & PIECE1) << SHIFT6) | ((face & PIECE2) << SHIFT4) |
        ((face & PIECE3) << SHIFT2) |
        ((face & PIECE4)) | ((face & PIECE5) >> SHIFT2) | ((face & PIECE6) >> SHIFT4) |
        ((face & PIECE7) >> SHIFT6) | ((face & PIECE8) >> SHIFT8)
    }


    fn right(&self) -> Self {
        Cube {
            up: (self.up & !MASK258) | (self.front & MASK258),
            down: (self.down & !MASK258) | (self.back & MASK258),
            left: self.left,
            right: Self::rotate_face(self.right),
            front: (self.front & !MASK258) | (self.down & MASK258),
            back: (self.back & !MASK258) | (self.up & MASK258),
        }
    }

    fn right_(&self) -> Self {
        Cube {
            up: (self.up & !MASK258) | (self.back & MASK258),
            down: (self.down & !MASK258) | (self.front & MASK258),
            left: self.left,
            right: Self::rotate_face_(self.right),
            front: (self.front & !MASK258) | (self.up & MASK258),
            back: (self.back & !MASK258) | (self.down & MASK258),
        }
    }

    fn right2(&self) -> Self {
        Cube {
            up: (self.up & !MASK258) | (self.down & MASK258),
            down: (self.down & !MASK258) | (self.up & MASK258),
            left: self.left,
            right: Self::rotate_face2(self.right),
            front: (self.front & !MASK258) | (self.back & MASK258),
            back: (self.back & !MASK258) | (self.front & MASK258),
        }
    }


    fn left(&self) -> Self {
        Cube {
            up: (self.up & !MASK036) | (self.back & MASK036),
            down: (self.down & !MASK036) | (self.front & MASK036),
            left: Self::rotate_face(self.left),
            right: self.right,
            front: (self.front & !MASK036) | (self.up & MASK036),
            back: (self.back & !MASK036) | (self.down & MASK036),
        }
    }

    fn left_(&self) -> Self {
        Cube {
            up: (self.up & !MASK036) | (self.front & MASK036),
            down: (self.down & !MASK036) | (self.back & MASK036),
            left: Self::rotate_face_(self.left),
            right: self.right,
            front: (self.front & !MASK036) | (self.down & MASK036),
            back: (self.back & !MASK036) | (self.up & MASK036),
        }
    }

    fn left2(&self) -> Self {
        Cube {
            up: (self.up & !MASK036) | (self.down & MASK036),
            down: (self.down & !MASK036) | (self.up & MASK036),
            left: Self::rotate_face2(self.left),
            right: self.right,
            front: (self.front & !MASK036) | (self.back & MASK036),
            back: (self.back & !MASK036) | (self.front & MASK036),
        }
    }


    fn middle(&self) -> Self {
        Cube {
            up: (self.up & !MASK147) | (self.back & MASK147),
            down: (self.down & !MASK147) | (self.front & MASK147),
            left: self.left,
            right: self.right,
            front: (self.front & !MASK147) | (self.up & MASK147),
            back: (self.back & !MASK147) | (self.down & MASK147),
        }
    }

    fn middle_(&self) -> Self {
        Cube {
            up: (self.up & !MASK147) | (self.front & MASK147),
            down: (self.down & !MASK147) | (self.back & MASK147),
            left: self.left,
            right: self.right,
            front: (self.front & !MASK147) | (self.down & MASK147),
            back: (self.back & !MASK147) | (self.up & MASK147),
        }
    }

    fn middle2(&self) -> Self {
        Cube {
            up: (self.up & !MASK147) | (self.down & MASK147),
            down: (self.down & !MASK147) | (self.up & MASK147),
            left: self.left,
            right: self.right,
            front: (self.front & !MASK147) | (self.back & MASK147),
            back: (self.back & !MASK147) | (self.front & MASK147),
        }
    }


    fn front(&self) -> Self {
        let right_to_down = ((self.right & PIECE6) >> SHIFT4) | ((self.right & PIECE7) >> SHIFT6) |
                            ((self.right & PIECE8) >> SHIFT8);

        let down_to_left = ((self.down & PIECE2) << SHIFT4) | ((self.down & PIECE1) << SHIFT6) |
                           ((self.down & PIECE0) << SHIFT8);

        Cube {
            up: (self.up & !MASK678) | (self.left & MASK678),
            down: (self.down & !MASK012) | right_to_down,
            left: (self.left & !MASK678) | down_to_left,
            right: (self.right & !MASK678) | (self.up & MASK678),
            front: Self::rotate_face(self.front),
            back: self.back,
        }
    }

    fn front_(&self) -> Self {
        let left_to_down = ((self.left & PIECE6) >> SHIFT4) | ((self.left & PIECE7) >> SHIFT6) |
                           ((self.left & PIECE8) >> SHIFT8);

        let down_to_right = ((self.down & PIECE2) << SHIFT4) | ((self.down & PIECE1) << SHIFT6) |
                            ((self.down & PIECE0) << SHIFT8);

        Cube {
            up: (self.up & !MASK678) | (self.right & MASK678),
            down: (self.down & !MASK012) | left_to_down,
            left: (self.left & !MASK678) | (self.up & MASK678),
            right: (self.right & !MASK678) | down_to_right,
            front: Self::rotate_face_(self.front),
            back: self.back,
        }
    }

    fn front2(&self) -> Self {
        let up_to_down = ((self.up & PIECE6) >> SHIFT4) | ((self.up & PIECE7) >> SHIFT6) |
                         ((self.up & PIECE8) >> SHIFT8);

        let down_to_up = ((self.down & PIECE2) << SHIFT4) | ((self.down & PIECE1) << SHIFT6) |
                         ((self.down & PIECE0) << SHIFT8);

        Cube {
            up: (self.up & !MASK678) | down_to_up,
            down: (self.down & !MASK012) | up_to_down,
            left: (self.left & !MASK678) | (self.right & MASK678),
            right: (self.right & !MASK678) | (self.left & MASK678),
            front: Self::rotate_face2(self.front),
            back: self.back,
        }
    }


    fn back(&self) -> Self {
        let down_to_right = ((self.down & PIECE6) >> SHIFT4) | ((self.down & PIECE7) >> SHIFT6) |
                            ((self.down & PIECE8) >> SHIFT8);

        let left_to_down = ((self.left & PIECE2) << SHIFT4) | ((self.left & PIECE1) << SHIFT6) |
                           ((self.left & PIECE0) << SHIFT8);

        Cube {
            up: (self.up & !MASK012) | (self.right & MASK012),
            down: (self.down & !MASK678) | left_to_down,
            left: (self.left & !MASK012) | (self.up & MASK012),
            right: (self.right & !MASK012) | down_to_right,
            front: self.front,
            back: Self::rotate_face(self.back),
        }
    }

    fn back_(&self) -> Self {
        let down_to_left = ((self.down & PIECE6) >> SHIFT4) | ((self.down & PIECE7) >> SHIFT6) |
                           ((self.down & PIECE8) >> SHIFT8);

        let right_to_down = ((self.right & PIECE2) << SHIFT4) | ((self.right & PIECE1) << SHIFT6) |
                            ((self.right & PIECE0) << SHIFT8);

        Cube {
            up: (self.up & !MASK012) | (self.left & MASK012),
            down: (self.down & !MASK678) | right_to_down,
            left: (self.left & !MASK012) | down_to_left,
            right: (self.right & !MASK012) | (self.up & MASK012),
            front: self.front,
            back: Self::rotate_face_(self.back),
        }
    }

    fn back2(&self) -> Self {
        let down_to_up = ((self.down & PIECE6) >> SHIFT4) | ((self.down & PIECE7) >> SHIFT6) |
                         ((self.down & PIECE8) >> SHIFT8);

        let up_to_down = ((self.up & PIECE2) << SHIFT4) | ((self.up & PIECE1) << SHIFT6) |
                         ((self.up & PIECE0) << SHIFT8);

        Cube {
            up: (self.up & !MASK012) | down_to_up,
            down: (self.down & !MASK678) | up_to_down,
            left: (self.left & !MASK012) | (self.right & MASK012),
            right: (self.right & !MASK012) | (self.left & MASK012),
            front: self.front,
            back: Self::rotate_face2(self.back),
        }
    }


    fn up(&self) -> Self {
        let right_to_front = ((self.right & PIECE0) << SHIFT2) | ((self.right & PIECE3) >> SHIFT2) |
                             ((self.right & PIECE6) >> SHIFT6);

        let front_to_left = ((self.front & PIECE2) << SHIFT6) | ((self.front & PIECE1) << SHIFT4) |
                            ((self.front & PIECE0) << SHIFT2);

        let left_to_back = ((self.left & PIECE8) >> SHIFT2) | ((self.left & PIECE5) << SHIFT2) |
                           ((self.left & PIECE2) << SHIFT6);

        let back_to_right = ((self.back & PIECE6) >> SHIFT6) | ((self.back & PIECE7) >> SHIFT4) |
                            ((self.back & PIECE8) >> SHIFT2);

        Cube {
            up: Self::rotate_face(self.up),
            down: self.down,
            left: (self.left & !MASK258) | front_to_left,
            right: (self.right & !MASK036) | back_to_right,
            front: (self.front & !MASK012) | right_to_front,
            back: (self.back & !MASK678) | left_to_back,
        }
    }

    fn up_(&self) -> Self {
        let right_to_back = ((self.right & PIECE0) << SHIFT6) | ((self.right & PIECE3) << SHIFT4) |
                            ((self.right & PIECE6) << SHIFT2);

        let front_to_right = ((self.front & PIECE2) >> SHIFT2) | ((self.front & PIECE1) << SHIFT2) |
                             ((self.front & PIECE0) << SHIFT6);

        let left_to_front = ((self.left & PIECE8) >> SHIFT6) | ((self.left & PIECE5) >> SHIFT4) |
                            ((self.left & PIECE2) >> SHIFT2);

        let back_to_left = ((self.back & PIECE6) << SHIFT2) | ((self.back & PIECE7) >> SHIFT2) |
                           ((self.back & PIECE8) >> SHIFT6);

        Cube {
            up: Self::rotate_face_(self.up),
            down: self.down,
            left: (self.left & !MASK258) | back_to_left,
            right: (self.right & !MASK036) | front_to_right,
            front: (self.front & !MASK012) | left_to_front,
            back: (self.back & !MASK678) | right_to_back,
        }
    }

    fn up2(&self) -> Self {
        let right_to_left = ((self.right & PIECE0) << SHIFT8) | ((self.right & PIECE3) << SHIFT2) |
                            ((self.right & PIECE6) >> SHIFT4);

        let front_to_back = ((self.front & PIECE2) << SHIFT4) | ((self.front & PIECE1) << SHIFT6) |
                            ((self.front & PIECE0) << SHIFT8);

        let left_to_right = ((self.left & PIECE8) >> SHIFT8) | ((self.left & PIECE5) >> SHIFT2) |
                            ((self.left & PIECE2) << SHIFT4);

        let back_to_front = ((self.back & PIECE6) >> SHIFT4) | ((self.back & PIECE7) >> SHIFT6) |
                            ((self.back & PIECE8) >> SHIFT8);

        Cube {
            up: Self::rotate_face2(self.up),
            down: self.down,
            left: (self.left & !MASK258) | right_to_left,
            right: (self.right & !MASK036) | left_to_right,
            front: (self.front & !MASK012) | back_to_front,
            back: (self.back & !MASK678) | front_to_back,
        }
    }


    fn down(&self) -> Self {
        let right_to_back = ((self.right & PIECE8) >> SHIFT6) | ((self.right & PIECE5) >> SHIFT4) |
                            ((self.right & PIECE2) >> SHIFT2);

        let front_to_right = ((self.front & PIECE6) << SHIFT2) | ((self.front & PIECE7) >> SHIFT2) |
                             ((self.front & PIECE8) >> SHIFT6);

        let left_to_front = ((self.left & PIECE0) << SHIFT6) | ((self.left & PIECE3) << SHIFT4) |
                            ((self.left & PIECE6) << SHIFT2);

        let back_to_left = ((self.back & PIECE2) >> SHIFT2) | ((self.back & PIECE1) << SHIFT2) |
                           ((self.back & PIECE0) << SHIFT6);

        Cube {
            up: self.up,
            down: Self::rotate_face(self.down),
            left: (self.left & !MASK036) | back_to_left,
            right: (self.right & !MASK258) | front_to_right,
            front: (self.front & !MASK678) | left_to_front,
            back: (self.back & !MASK012) | right_to_back,
        }
    }

    fn down_(&self) -> Self {
        let back_to_right = ((self.back & PIECE2) << SHIFT6) | ((self.back & PIECE1) << SHIFT4) |
                            ((self.back & PIECE0) << SHIFT2);

        let right_to_front = ((self.right & PIECE8) >> SHIFT2) | ((self.right & PIECE5) << SHIFT2) |
                             ((self.right & PIECE2) << SHIFT6);

        let front_to_left = ((self.front & PIECE6) >> SHIFT6) | ((self.front & PIECE7) >> SHIFT4) |
                            ((self.front & PIECE8) >> SHIFT2);

        let left_to_back = ((self.left & PIECE0) << SHIFT2) | ((self.left & PIECE3) >> SHIFT2) |
                           ((self.left & PIECE6) >> SHIFT6);

        Cube {
            up: self.up,
            down: Self::rotate_face_(self.down),
            left: (self.left & !MASK036) | front_to_left,
            right: (self.right & !MASK258) | back_to_right,
            front: (self.front & !MASK678) | right_to_front,
            back: (self.back & !MASK012) | left_to_back,
        }
    }

    fn down2(&self) -> Self {
        let left_to_right = ((self.left & PIECE0) << SHIFT8) | ((self.left & PIECE3) << SHIFT2) |
                            ((self.left & PIECE6) >> SHIFT4);

        let back_to_front = ((self.back & PIECE2) << SHIFT4) | ((self.back & PIECE1) << SHIFT6) |
                            ((self.back & PIECE0) << SHIFT8);

        let right_to_left = ((self.right & PIECE8) >> SHIFT8) | ((self.right & PIECE5) >> SHIFT2) |
                            ((self.right & PIECE2) << SHIFT4);

        let front_to_back = ((self.front & PIECE6) >> SHIFT4) | ((self.front & PIECE7) >> SHIFT6) |
                            ((self.front & PIECE8) >> SHIFT8);

        Cube {
            up: self.up,
            down: Self::rotate_face2(self.down),
            left: (self.left & !MASK036) | right_to_left,
            right: (self.right & !MASK258) | left_to_right,
            front: (self.front & !MASK678) | back_to_front,
            back: (self.back & !MASK012) | front_to_back,
        }
    }


    pub fn turn(&self, t: Turn) -> Self {
        use self::Turn::*;

        match t {
            U => self.up(),
            U_ => self.up_(),
            U2 => self.up2(),
            D => self.down(),
            D_ => self.down_(),
            D2 => self.down2(),
            L => self.left(),
            L_ => self.left_(),
            L2 => self.left2(),
            R => self.right(),
            R_ => self.right_(),
            R2 => self.right2(),
            F => self.front(),
            F_ => self.front_(),
            F2 => self.front2(),
            B => self.back(),
            B_ => self.back_(),
            B2 => self.back2(),
            M => self.middle(),
            M_ => self.middle_(),
            M2 => self.middle2(),
        }
    }
}

fn nth_chunk(n: usize, face: u32) -> Color {
    use self::Color::*;

    match (face >> (n * 3)) & 0b111 {
        0 => Grey,
        1 => White,
        2 => Yellow,
        3 => Green,
        4 => Blue,
        5 => Red,
        6 => Orange,
        x => panic!("Invalid chunk {} in face {} at {}", x, face, n),
    }
}

impl fmt::Display for Cube {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // Back

        writeln!(f,
                 "        {}{}{}\n        {}{}{}\n        {}{}{}",
                 nth_chunk(0, self.back),
                 nth_chunk(1, self.back),
                 nth_chunk(2, self.back),

                 nth_chunk(3, self.back),
                 nth_chunk(4, self.back),
                 nth_chunk(5, self.back),

                 nth_chunk(6, self.back),
                 nth_chunk(7, self.back),
                 nth_chunk(8, self.back))
            .unwrap();

        // Down, left, up, right

        writeln!(f,
                 "{}{}{} {}{}{} {}{}{} {}{}{}",
                 nth_chunk(8, self.down),
                 nth_chunk(7, self.down),
                 nth_chunk(6, self.down),

                 nth_chunk(0, self.left),
                 nth_chunk(1, self.left),
                 nth_chunk(2, self.left),

                 nth_chunk(0, self.up),
                 nth_chunk(1, self.up),
                 nth_chunk(2, self.up),

                 nth_chunk(0, self.right),
                 nth_chunk(1, self.right),
                 nth_chunk(2, self.right),
        )
            .unwrap();

        writeln!(f,
                 "{}{}{} {}{}{} {}{}{} {}{}{}",
                 nth_chunk(5, self.down),
                 nth_chunk(4, self.down),
                 nth_chunk(3, self.down),

                 nth_chunk(3, self.left),
                 nth_chunk(4, self.left),
                 nth_chunk(5, self.left),

                 nth_chunk(3, self.up),
                 nth_chunk(4, self.up),
                 nth_chunk(5, self.up),

                 nth_chunk(3, self.right),
                 nth_chunk(4, self.right),
                 nth_chunk(5, self.right),
        )
            .unwrap();


        writeln!(f,
                 "{}{}{} {}{}{} {}{}{} {}{}{}",
                 nth_chunk(2, self.down),
                 nth_chunk(1, self.down),
                 nth_chunk(0, self.down),

                 nth_chunk(6, self.left),
                 nth_chunk(7, self.left),
                 nth_chunk(8, self.left),

                 nth_chunk(6, self.up),
                 nth_chunk(7, self.up),
                 nth_chunk(8, self.up),

                 nth_chunk(6, self.right),
                 nth_chunk(7, self.right),
                 nth_chunk(8, self.right),
        )
            .unwrap();

        // Front

        writeln!(f,
                 "        {}{}{}\n        {}{}{}\n        {}{}{}",
                 nth_chunk(0, self.front),
                 nth_chunk(1, self.front),
                 nth_chunk(2, self.front),

                 nth_chunk(3, self.front),
                 nth_chunk(4, self.front),
                 nth_chunk(5, self.front),

                 nth_chunk(6, self.front),
                 nth_chunk(7, self.front),
                 nth_chunk(8, self.front))
    }
}

fn search_helper(
    cube: Cube,
    last_turn: u8,
    depth: usize,
    max_depth: usize,
    pattern: &Cube,
    history: &mut [Turn],
    allowed_turns: &[Turn],
    tx: &Sender<SearchResult>
) {
    if depth > max_depth {
        return;
    }

    if depth == max_depth && cube.matches(pattern) {
        let alg = history.iter().take(depth).map(|&turn| turn).collect();

        match tx.send(SearchResult::Algorithm(alg)) {
            Ok(()) => {}
            Err(_) => return,
        }

        return;
    }

    for &turn in allowed_turns.iter() {
        if turn as u8 ^ last_turn > 0b11 {
            history[depth] = turn;
            search_helper(cube.turn(turn),
                          turn as u8,
                          depth + 1,
                          max_depth,
                          pattern,
                          history,
                          allowed_turns,
                          tx);
        }
    }

}

pub fn search(cube: Cube, pattern: &Cube, allowed_turns: &[Turn], tx: Sender<SearchResult>) {
    let mut max_depth = 1;

    loop {
        match tx.send(SearchResult::Depth(max_depth)) {
            Ok(()) => {}
            Err(_) => return,
        }

        let senders: Vec<_> = allowed_turns.iter().map(|_| tx.clone()).collect();

        allowed_turns.into_par_iter().zip(senders).for_each(move |(&turn, sender)| {
            let mut history = vec![turn; max_depth+1];
            let cube = cube.turn(turn);

            search_helper(cube,
                          turn as u8,
                          1,
                          max_depth,
                          pattern,
                          &mut history,
                          allowed_turns,
                          &sender);
        });

        max_depth += 1;
    }
}
