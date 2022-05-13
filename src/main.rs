pub mod game;
pub mod ui;
pub mod vector;

pub mod unit {
    use uom::system;
    use uom::ISQ;

    ISQ!(
        uom::si,
        f32,
        (foot, kilogram, second, ampere, kelvin, mole, candela)
    );
}

fn main() {
    println!("Hello, world!");
}
