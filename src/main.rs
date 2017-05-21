#[macro_use]
extern crate conrod;
extern crate clipboard;

use conrod::{widget, Colorable, Positionable, Widget, Sizeable, Borderable, Labelable};
use conrod::backend::glium::glium::{self, DisplayBuild, Surface};

use clipboard::ClipboardProvider;
use clipboard::ClipboardContext;

use std::thread;
use std::sync::mpsc::{channel, Sender};

pub mod cube;

use cube::*;

type PieceColors = Cube<[conrod::Color; 9]>;

const WIDTH: u32 = 1024;
const HEIGHT: u32 = 768;

const COLORS: [conrod::Color; 7] = [conrod::color::WHITE,
                                    conrod::color::RED,
                                    conrod::color::BLUE,
                                    conrod::color::YELLOW,
                                    conrod::color::GREEN,
                                    conrod::color::ORANGE,
                                    conrod::color::GREY];

const DEFAULT_PIECE_COLORS: PieceColors = PieceColors {
    up: [conrod::color::YELLOW; 9],
    down: [conrod::color::WHITE; 9],
    left: [conrod::color::RED; 9],
    right: [conrod::color::ORANGE; 9],
    front: [conrod::color::GREEN; 9],
    back: [conrod::color::BLUE; 9],
};

fn to_cube_color(color: &conrod::Color) -> Color {
    use conrod::color::*;
    use cube::Color;

    let color = *color;

    if color == YELLOW {
        Color::Yellow
    } else if color == WHITE {
        Color::White
    } else if color == RED {
        Color::Red
    } else if color == ORANGE {
        Color::Orange
    } else if color == BLUE {
        Color::Blue
    } else if color == GREEN {
        Color::Green
    } else if color == GREY {
        Color::Grey
    } else {
        panic!("Invalid color: {:?}", color)
    }
}

fn to_cube(colors: &PieceColors) -> Cube {
    let mut down: Vec<Color> = colors.down.iter().map(to_cube_color).collect();
    down.reverse();

    let cube: Cube<Vec<Color>> = Cube {
        up: colors.up.iter().map(to_cube_color).collect(),
        down: down,
        left: colors.left.iter().map(to_cube_color).collect(),
        right: colors.right.iter().map(to_cube_color).collect(),
        front: colors.front.iter().map(to_cube_color).collect(),
        back: colors.back.iter().map(to_cube_color).collect(),
    };

    cube.pack()
}

fn search_helper(
    from: Cube,
    to: Cube,
    allowed_turns: Vec<(Turn, bool)>,
    tx: Sender<SearchResult>
) {
    let allowed: Vec<Turn> = allowed_turns.iter()
        .filter_map(|&(turn, b)| if b { Some(turn) } else { None })
        .collect();

    search(from, &to, &allowed, tx);
}


pub fn main() {
    use cube::Turn::*;

    let mut allowed_turns = vec![(U, true),
                                 (U_, true),
                                 (U2, true),
                                 (D, false),
                                 (D_, false),
                                 (D2, false),
                                 (L, true),
                                 (L_, true),
                                 (L2, true),
                                 (R, true),
                                 (R_, true),
                                 (R2, true),
                                 (F, true),
                                 (F_, true),
                                 (F2, true),
                                 (B, false),
                                 (B_, false),
                                 (B2, false),
                                 (M, true),
                                 (M_, true),
                                 (M2, true)];

    let mut searching = false;
    let mut search_results: Vec<SearchResult> = Vec::new();
    let (mut algs_tx, mut algs_rx) = channel();

    let mut clipboard: ClipboardContext = ClipboardProvider::new().unwrap();

    // Build the window.
    let display = glium::glutin::WindowBuilder::new()
        .with_vsync()
        .with_dimensions(WIDTH, HEIGHT)
        .with_title("Rubik's Cube Algorithm Finder")
        .with_multisampling(4)
        .build_glium()
        .unwrap();


    let mut ui = conrod::UiBuilder::new([WIDTH as f64, HEIGHT as f64]).build();

    widget_ids!(struct Ids {
        container, left_pane, right_pane,
        canvas_from, canvas_to, from_faces, to_faces,
        color_picker_list, color_picker,
        canvas_algorithms, list_algorithms,
        controls, search_button, reset_state_button, reset_goal_button,
        allowed_turns, allowed_turns_list,
    });

    let ids = Ids::new(ui.widget_id_generator());

    const FONT_PATH: &'static str = concat!(env!("CARGO_MANIFEST_DIR"),
                                            "/assets/fonts/NotoSans/NotoSans-Regular.ttf");
    ui.fonts.insert_from_file(FONT_PATH).unwrap();

    let mut renderer = conrod::backend::glium::Renderer::new(&display).unwrap();

    let image_map = conrod::image::Map::<glium::texture::Texture2d>::new();

    let mut last_update = std::time::Instant::now();
    let mut ui_needs_update = true;

    let mut from_colors = DEFAULT_PIECE_COLORS;
    let mut to_colors = DEFAULT_PIECE_COLORS;

    let mut current_color = conrod::color::GREY;

    let sixteen_ms = std::time::Duration::from_millis(16);

    'main: loop {
        let duration_since_last_update = std::time::Instant::now().duration_since(last_update);
        if duration_since_last_update < sixteen_ms {
            std::thread::sleep(sixteen_ms - duration_since_last_update);
        }

        match algs_rx.try_recv() {
            Ok(res) => {
                search_results.push(res);
                ui_needs_update = true;
            }
            Err(_) => {}
        }

        let events: Vec<_> = display.poll_events().collect();

        if events.is_empty() && !ui_needs_update {
            last_update = std::time::Instant::now();
            continue;
        }

        ui_needs_update = false;
        last_update = std::time::Instant::now();

        for event in events {
            if let Some(event) = conrod::backend::winit::convert(event.clone(), &display) {
                ui.handle_event(event);
                ui_needs_update = true;
            }

            match event {
                glium::glutin::Event::Closed => break 'main,
                _ => {}
            }
        }

        {
            let ui = &mut ui.set_widgets();

            let facedim = ui.win_h / 6.5;

            let lpane = [(ids.canvas_from,
                          widget::Canvas::new()
                              .color(conrod::color::WHITE)
                              .length(3.0 * facedim)),
                         (ids.color_picker,
                          widget::Canvas::new()
                              .color(conrod::color::WHITE)
                              .length(0.5 * facedim)),
                         (ids.canvas_to,
                          widget::Canvas::new()
                              .color(conrod::color::WHITE)
                              .length(3.0 * facedim))];

            let rpane = [(ids.controls,
                          widget::Canvas::new()
                              .length_weight(0.1)
                              .color(conrod::color::WHITE)),
                         (ids.canvas_algorithms,
                          widget::Canvas::new().color(conrod::color::WHITE))];

            widget::Canvas::new()
                .wh_of(ui.window)
                .middle_of(ui.window)
                .flow_right(&[(ids.left_pane,
                               widget::Canvas::new()
                                   .length(4.0 * facedim)
                                   .flow_down(&lpane)),
                              (ids.right_pane, widget::Canvas::new().flow_down(&rpane)),
                              (ids.allowed_turns, widget::Canvas::new().length_weight(0.15))])
                .set(ids.container, ui);

            // Cube

            let from = to_cube(&from_colors);
            let to = to_cube(&to_colors);

            let missing_colors = from.missing_colors(&to);

            // Color picker

            let color_padding_h = 0.15 * 0.5 * facedim;
            let color_padding_w = (4.0 * facedim / COLORS.len() as conrod::Scalar -
                                   0.7 * 0.5 * facedim) / 2.0;

            let mut matrix = widget::Matrix::new(COLORS.len(), 1)
                .middle_of(ids.color_picker)
                .wh_of(ids.color_picker)
                .cell_padding(color_padding_w, color_padding_h)
                .set(ids.color_picker_list, ui);

            while let Some(item) = matrix.next(ui) {
                let color = COLORS[item.col];
                let cube_color = to_cube_color(&color);
                let missing = missing_colors.contains(&cube_color);

                let border_color = if missing && color != current_color {
                    conrod::color::WHITE
                } else {
                    conrod::color::BLACK
                };

                let border = if missing || color == current_color {
                    5.0
                } else {
                    1.0
                };

                let button = widget::Button::new()
                    .color(color)
                    .border(border)
                    .border_color(border_color);

                if item.set(button, ui).was_clicked() {
                    current_color = color;
                }
            }

            // Controls

            let controls_font_size = (0.025 * ui.win_w) as u32;
            let control_w = ui.w_of(ids.controls).unwrap_or_default() / 3.0;

            if widget::Button::new()
                .w(control_w)
                .h_of(ids.controls)
                .mid_left_of(ids.controls)
                .label(if searching { "Stop" } else { "Search" })
                .label_font_size(controls_font_size)
                .set(ids.search_button, ui)
                .was_clicked() {
                if searching {
                    searching = false;
                    let (new_tx, new_rx) = channel();
                    algs_tx = new_tx;
                    algs_rx = new_rx;
                } else {
                    if missing_colors.is_empty() {
                        searching = true;
                        search_results.clear();
                        let turns = allowed_turns.clone();
                        let tx = algs_tx.clone();

                        thread::spawn(move || { search_helper(from, to, turns, tx); });
                    }
                }
            }

            if widget::Button::new()
                .w(control_w)
                .h_of(ids.controls)
                .middle_of(ids.controls)
                .label("Reset state")
                .label_font_size(controls_font_size)
                .set(ids.reset_state_button, ui)
                .was_clicked() {
                from_colors = DEFAULT_PIECE_COLORS;
            }

            if widget::Button::new()
                .w(control_w)
                .h_of(ids.controls)
                .mid_right_of(ids.controls)
                .label("Reset goal")
                .label_font_size(controls_font_size)
                .set(ids.reset_goal_button, ui)
                .was_clicked() {
                to_colors = DEFAULT_PIECE_COLORS;
            }

            // Allowed turns

            let (mut items, _) = widget::List::flow_down(allowed_turns.len())
                .item_size(ui.win_h / allowed_turns.len() as conrod::Scalar)
                .scrollbar_on_top()
                .middle_of(ids.allowed_turns)
                .wh_of(ids.allowed_turns)
                .set(ids.allowed_turns_list, ui);

            while let Some(item) = items.next(ui) {
                let (turn, allowed) = allowed_turns[item.i];
                let label = format!("{}", turn);

                let toggle = widget::Toggle::new(allowed)
                    .label(&label)
                    .label_color(conrod::color::WHITE)
                    .label_font_size((0.025 * ui.win_h) as u32)
                    .color(conrod::color::LIGHT_BLUE);

                for v in item.set(toggle, ui) {
                    allowed_turns[item.i] = (turn, v);
                }
            }

            // Search results

            let alg_font_size = std::cmp::min((0.03 * ui.win_w) as u32, 24);
            let depth_font_size = std::cmp::min((0.032 * ui.win_w) as u32, 28);

            let (mut items, scrollbar) = widget::List::flow_down(search_results.len())
                .item_size(1.6 * alg_font_size as conrod::Scalar)
                .scrollbar_on_top()
                .middle_of(ids.canvas_algorithms)
                .padded_wh_of(ids.canvas_algorithms, 15.0)
                .set(ids.list_algorithms, ui);

            while let Some(item) = items.next(ui) {
                let mut label = String::new();
                let mut label_clone = String::new();

                let button = match &search_results[item.i] {
                    &SearchResult::Algorithm(ref alg) => {
                        for turn in alg {
                            label.push_str(&format!(" {}", turn));
                        }

                        label_clone = label.clone();

                        widget::Button::new()
                            .label(&label)
                            .label_font_size(alg_font_size)
                            .label_x(conrod::position::Relative::Align(
                                conrod::position::Align::Start
                            ))
                            .color(conrod::color::WHITE)
                            .border(0.0)

                    }
                    &SearchResult::Depth(d) => {
                        label.push_str(&format!("{}", d));

                        widget::Button::new()
                            .label(&label)
                            .label_color(conrod::color::LIGHT_BLUE)
                            .label_font_size(depth_font_size)
                            .border(0.0)
                    }
                };

                if item.set(button, ui).was_clicked() && !label_clone.is_empty() {
                    match clipboard.set_contents(label_clone) {
                        Ok(()) => {}
                        Err(e) => println!("Failed to copy to clipboard: {}", e),
                    }
                }
            }

            if let Some(s) = scrollbar {
                s.set(ui)
            }

            // From

            let face_padding = 0.025 * ui.w_of(ids.canvas_from).unwrap_or_default();

            let mut from_faces = widget::Matrix::new(4, 3)
                .middle_of(ids.canvas_from)
                .padded_wh_of(ids.canvas_from, 0.1 * facedim)
                .cell_padding(face_padding, face_padding)
                .set(ids.from_faces, ui);

            fill_face(&mut from_faces, &mut from_colors, ui, current_color);

            // To

            let mut to_faces = widget::Matrix::new(4, 3)
                .middle_of(ids.canvas_to)
                .padded_wh_of(ids.canvas_from, 0.1 * facedim)
                .cell_padding(face_padding, face_padding)
                .set(ids.to_faces, ui);

            fill_face(&mut to_faces, &mut to_colors, ui, current_color);
        }


        if let Some(primitives) = ui.draw_if_changed() {
            renderer.fill(&display, primitives, &image_map);
            let mut target = display.draw();
            target.clear_color(1.0, 1.0, 1.0, 1.0);
            renderer.draw(&display, &mut target, &image_map).unwrap();
            target.finish().unwrap();
        }
    }
}

fn fill_face(
    faces: &mut conrod::widget::matrix::Elements,
    piece_colors: &mut PieceColors,
    ui: &mut conrod::UiCell,
    current_color: conrod::Color
) {
    let mut colors_list = [[None, None, Some(&mut piece_colors.back), None],
                           [Some(&mut piece_colors.down),
                            Some(&mut piece_colors.left),
                            Some(&mut piece_colors.up),
                            Some(&mut piece_colors.right)],
                           [None, None, Some(&mut piece_colors.front), None]];

    while let Some(item) = faces.next(ui) {
        if let Some(ref mut colors) = colors_list[item.row][item.col] {
            let mut face = item.set(widget::Matrix::new(3, 3), ui);

            while let Some(piece) = face.next(ui) {
                let i = 3 * piece.row + piece.col;

                if piece.set(widget::Button::new().color(colors[i]), ui)
                    .was_clicked() {
                    colors[i] = current_color;
                }
            }
        }
    }
}
