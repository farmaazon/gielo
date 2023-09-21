slint::include_modules!();

impl<'a> Functions<'a> {
    pub fn initialize(&self) {
        self.on_round(|x, precision| {
            let factor = (10.0 as i32).pow(precision as u32) as f32;
            (x * factor).round() / factor
        });
    }
}
