# algfinder

Graphical Rubik's Cube algorithm finder in Rust

Specify a starting state, a goal state, the allowed turns and press search.
The program will then find every algorithm, the shortest first, that solve the case.

# Features

- Grey color for pieces you don't care about
- Parallel search to utilise all your cores
- Toggles to select exactly the turns you want in your algorithms
- Shows you any colors you have too few of in the starting state
- Copy algorithms by clicking on them

# Screenshot

Here we are searching for solutions to the classical "Sune" case.

![Screenshot of the program](https://raw.githubusercontent.com/andreasfrom/algfinder/master/screenshot.png)
