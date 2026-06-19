use crate::{Delivery, sheet, simulation, unit};
use gielo_sheet::stone::{self, Stones};
use gielo_simulation::{Simulation, delivery::Process};
use gielo_team::{PerTeam, Player};

pub struct HeatmapFactory {
    pixels_per_foot: unit::BaseType,
    bitmap_width: u32,
    position_chance_buf: Vec<usize>,
    displacing_chance_buf: [usize; stone::COUNT],
    removing_chance_buf: [usize; stone::COUNT],
}

impl HeatmapFactory {
    pub fn new(bitmap_width: u32, sheet: sheet::Parameters) -> Self {
        let pixels_per_foot =
            bitmap_width as unit::BaseType / sheet.geometry.width.get::<unit::length::foot>();
        let bitmap_coverage_height = sheet.geometry.length - sheet.geometry.playing_end.hog_line_y;
        let bitmap_height =
            (bitmap_coverage_height * pixels_per_foot).get::<unit::length::foot>().ceil() as u32;
        Self {
            pixels_per_foot,
            bitmap_width,
            position_chance_buf: vec![0; (bitmap_width * bitmap_height) as usize],
            displacing_chance_buf: [0; stone::COUNT],
            removing_chance_buf: [0; stone::COUNT],
        }
    }

    pub fn create<'a, 'b, 'c>(
        &'a mut self,
        current_stones: &'b Stones,
        played_stone: stone::Id,
        sheet: &'c sheet::Parameters,
    ) -> Heatmap<'a, 'b, 'c> {
        self.position_chance_buf.fill(0);
        self.displacing_chance_buf.fill(0);
        self.removing_chance_buf.fill(0);
        Heatmap {
            current_stones,
            played_stone,
            sheet,
            bitmap_width: self.bitmap_width,
            pixels_per_foot: self.pixels_per_foot,
            position_chance: &mut self.position_chance_buf,
            leave_chance: 0,
            displacing_chance: &mut self.displacing_chance_buf,
            removing_chance: &mut self.removing_chance_buf,
            probes_count: 0,
        }
    }
}

pub struct Heatmap<'a, 'b, 'c> {
    pub current_stones: &'b Stones,
    pub played_stone: stone::Id,
    pub sheet: &'c sheet::Parameters,
    pub bitmap_width: u32,
    pixels_per_foot: unit::BaseType,
    pub position_chance: &'a mut [usize],
    pub leave_chance: usize,
    pub displacing_chance: &'a mut [usize; stone::COUNT],
    pub removing_chance: &'a mut [usize; stone::COUNT],
    pub probes_count: usize,
}

impl<'a, 'b, 'c> Heatmap<'a, 'b, 'c> {
    pub fn apply_result(&mut self, stones: Stones) {
        if stones.in_play().contains(self.played_stone) {
            let pos = stones.positions()[self.played_stone];
            let y_on_bmp = pos.y - self.sheet.geometry.playing_end.hog_line_y;
            let r = self.sheet.stone_radius;
            let min_x = u32::max(self.length_to_pixels(pos.x - r).floor() as u32, 0);
            let max_x = u32::min(self.length_to_pixels(pos.x + r).ceil() as u32, self.bitmap_width);
            let min_y = u32::max(self.length_to_pixels(y_on_bmp - r).floor() as u32, 0);
            let max_y =
                u32::min(self.length_to_pixels(y_on_bmp + r).ceil() as u32, self.bitmap_height());
            for x in min_x..=max_x {
                for y in min_y..=max_y {
                    let length_x = self.pixels_to_length(x as unit::BaseType);
                    let length_y = self.pixels_to_length(y as unit::BaseType);
                    if length_x * length_x + length_y * length_y <= r * r {
                        self.position_chance[(y * self.bitmap_width + x) as usize] += 1
                    }
                }
            }
        } else {
            self.leave_chance += 1;
        }
        for (stone, pos) in self.current_stones.iter_in_play() {
            if !stones.in_play().contains(stone) {
                self.displacing_chance[stone] += 1;
                self.removing_chance[stone] += 1;
            } else if stones.positions()[stone] != pos {
                self.displacing_chance[stone] += 1;
            }
        }
        self.probes_count += 1;
    }

    fn length_to_pixels(&self, length: unit::Length) -> unit::BaseType {
        length.get::<unit::length::foot>() * self.pixels_per_foot
    }

    fn pixels_to_length(&self, pixels: unit::BaseType) -> unit::Length {
        unit::feet(pixels / self.pixels_per_foot)
    }

    fn bitmap_height(&self) -> u32 {
        self.position_chance.len() as u32 / self.bitmap_width
    }

    pub fn create_pixel_buffer(
        &self,
        color: slint::Color,
    ) -> slint::SharedPixelBuffer<slint::Rgba8Pixel> {
        let mut retval = slint::SharedPixelBuffer::new(self.bitmap_width, self.bitmap_height());
        let buffer = retval.make_mut_bytes();
        for (rgba, pos_chance) in buffer.chunks_exact_mut(4).zip(self.position_chance.iter()) {
            for (buf_component, color_component) in
                rgba.iter_mut().zip([color.red(), color.green(), color.blue(), color.alpha()])
            {
                *buf_component = (color_component as usize * *pos_chance / self.probes_count) as u8;
            }
        }
        retval
    }

    pub fn stones_colors(
        &self,
        team_colors: PerTeam<slint::Color>,
    ) -> [slint::Color; stone::COUNT] {
        let mut colors = [slint::Color::default(); stone::COUNT];
        for id in 0..stone::COUNT {
            let color = team_colors[crate::team::stone::team(id)];
            let alpha =
                (color.alpha() as usize * self.displacing_chance[id] / self.probes_count) as u8;
            colors[id] = slint::Color::from_argb_u8(alpha, color.red(), color.green(), color.blue())
        }
        colors
    }
}

pub fn heatmap_of_delivery_chances<'a, 'b, 'c, Name>(
    factory: &'a mut HeatmapFactory,
    current_stones: &'b Stones,
    played_stone: stone::Id,
    sheet: &'c sheet::Parameters,
    delivering_player: &Player<Name>,
    plan: Delivery,
    probes_count: usize,
    simulation: &Simulation,
) -> Heatmap<'a, 'b, 'c> {
    let mut heatmap = factory.create(current_stones, played_stone, sheet);
    for _probe_id in 0..probes_count {
        let actual = plan.apply_error(&delivering_player.skills);
        let mut stones_copy = current_stones.clone();
        let process_params = simulation::delivery::StartingConditions {
            stone: played_stone,
            angle: actual.angle,
            velocity: actual.velocity,
            hack: delivering_player.used_hack,
            rotation: actual.rotation,
        };
        let mut dirty = stone::Flag::default();
        let mut process = Process::new(process_params, &mut stones_copy, sheet, &mut dirty);
        let mut update = simulation::delivery::Update {
            process: &mut process,
            sheet: &mut stones_copy,
            sheet_params: sheet,
            simulation,
            dirty: &mut dirty,
        };
        update.run(None);
        heatmap.apply_result(stones_copy);
    }
    heatmap
}
