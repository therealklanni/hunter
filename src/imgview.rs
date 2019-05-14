use image;
use image::Pixel;
use image::FilterType;
use image::DynamicImage;
use image::GenericImageView;

use termion::color::{Bg, Fg, Rgb};

use crate::widget::{Widget, WidgetCore};
use crate::fail::{HResult, ErrorLog};

use std::path::Path;

impl std::cmp::PartialEq for ImgView {
    fn eq(&self, other: &Self) -> bool {
        self.core == other.core &&
            self.buffer == other.buffer
    }
}

pub struct ImgView {
    pub core: WidgetCore,
    pub buffer: Vec<String>,
    pub raw: DynamicImage
}

impl ImgView {
    pub fn new_from_file(core: WidgetCore, file: &Path) -> ImgView {
        let img = image::open(&file)
            .unwrap();

        ImgView {
            core: core,
            buffer: vec![],
            raw: img
        }
    }

    pub fn render(&mut self) {
        let (xsize, ysize) = self.max_size();

        let img = self.raw
            .resize_exact(xsize as u32,
                          ysize as u32,
                          FilterType::Nearest)
            .to_rgba();


        let rows = img.pixels()
            .collect::<Vec<_>>()
            .chunks(xsize as usize)
            .map(|line| line.to_vec())
            .collect::<Vec<Vec<_>>>();

        let buffer = rows
            .chunks(2)
            .map(|rows| {
                rows[0]
                    .iter()
                    .zip(rows[1].iter())
                    .map(|(upper, lower)| {
                        let upper_color = upper.to_rgb();
                        let lower_color = lower.to_rgb();

                        format!("{}{}▀",
                                Fg(Rgb(upper_color[0], upper_color[1], upper_color[2])),
                                Bg(Rgb(lower_color[0], lower_color[1], lower_color[2])))
                    }).collect()

            }).collect();

        self.buffer = buffer;
    }

    pub fn max_size(&self) -> (u32, u32) {
        let (xsize, _) = self.core.coordinates.size_u();
        let img_xsize = self.raw.width();
        let img_ysize = self.raw.height();
        let img_ratio = img_xsize as f32 / img_ysize as f32;

        let mut new_y = if img_ratio < 1 as f32 {
            (xsize+1) as f32 * img_ratio
        } else {
            (xsize+1) as f32 / img_ratio
        };
        if new_y as u32 % 2 == 1 {
            new_y += 1 as f32;
        }
        ((xsize+1) as u32, new_y as u32)
    }

    pub fn set_raw_img(&mut self, img: DynamicImage) {
        self.raw = img;
        self.render();
        self.draw().log();
    }
}


impl Widget for ImgView {
    fn get_core(&self) -> HResult<&WidgetCore> {
        Ok(&self.core)
    }

    fn get_core_mut(&mut self) -> HResult<&mut WidgetCore> {
        Ok(&mut self.core)
    }

    fn refresh(&mut self) -> HResult<()> {
        if self.buffer.len() == 0 {
            self.render();
        }
        Ok(())
    }

    fn get_drawlist(&self) -> HResult<String> {
        let (xpos, ypos) = self.core.coordinates.position_u();

        let draw = self.buffer
            .iter()
            .enumerate()
            .fold(String::new(), |mut draw, (pos, line)| {
                draw += &format!("{}", crate::term::goto_xy_u(xpos,
                                                              ypos + pos));
                draw += line;
                draw
            });

        Ok(draw)
    }
}
