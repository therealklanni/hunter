use image;
use image::Pixel;
use image::FilterType;
use image::DynamicImage;
use image::GenericImageView;

use termion::color::{Bg, Fg, Rgb};
use termion::input::TermRead;
use termion::event::Key;


use gstreamer::{self, prelude::*};
use gstreamer_app;

use failure;
use failure::Fail;

use rayon::prelude::*;

use std::io::Write;

pub type MResult<T> = Result<T, MediaError>;

#[derive(Fail, Debug)]
pub enum MediaError {
    #[fail(display = "Gstreamer failed!")]
    GstreamerError
}

fn main() {
    let args = std::env::args().collect::<Vec<String>>();
    let xsize: usize = args.get(1)
        .expect("Provide xsize")
        .parse::<usize>()
        .unwrap();
    let ysize = args.get(2)
        .expect("provide ysize")
        .parse()
        .unwrap();
    let video = args.get(3)
        .expect("Provide preview type")
        .parse::<usize>()
        .map(|option| option == 1)
        .unwrap();
    let autoplay = args.get(4)
        .expect("Autoplay?")
        .parse::<bool>()
        .unwrap();
    let mute = args.get(5)
        .expect("Muted?")
        .parse::<bool>()
        .unwrap();
    let path = args.get(6).expect("Provide path");

    if video == true {
        video_preview(path, xsize, ysize, autoplay, mute)
    } else {
        image_preview(path, xsize, ysize)
    }
}

fn image_preview(path: &str,
                 xsize: usize,
                 ysize: usize) {
    let img = image::open(&path).unwrap();

    let renderer = Renderer {
        xsize,
        ysize
    };

    renderer.send_image(img).unwrap();
}

fn video_preview(path: &String,
                 xsize: usize,
                 ysize: usize,
                 autoplay: bool,
                 mute: bool) {
    gstreamer::init().unwrap();

    let player = gstreamer::ElementFactory::make("playbin", None)
    //.ok_or(VideoError::GstreamerCreationError("playbin"))?;
        .unwrap();
    let videorate = gstreamer::ElementFactory::make("videorate", None)
        .unwrap();
        // .ok_or(VideoError::GstCreationError("videorate"))?;
    let pnmenc = gstreamer::ElementFactory::make("pnmenc", None)
    // .ok_or(VideoError::GstreamerCreationError("pnmenc"))?;
        .unwrap();
    let sink = gstreamer::ElementFactory::make("appsink", None)
    // .ok_or(VideoError::GstreamerCreationError("appsink"))?;
        .unwrap();
    let appsink = sink.clone()
        .downcast::<gstreamer_app::AppSink>()
        .unwrap();

    videorate.set_property("max-rate", &60).unwrap();

    let elems = &[&videorate, &pnmenc, &sink];

    let bin = gstreamer::Bin::new(None);
    bin.add_many(elems).unwrap();
    gstreamer::Element::link_many(elems).unwrap();

    // make input for bin point to first element
    let sink = elems[0].get_static_pad("sink").unwrap();
    let ghost = gstreamer::GhostPad::new("sink", &sink)
    // .ok_or(VideoError::GstCreationError("ghost pad"))?;
        .unwrap();
    ghost.set_active(true).unwrap();
    bin.add_pad(&ghost).unwrap();

    let uri = format!("file://{}", &path);

    player.set_property("uri", &uri).unwrap();
    player.set_property("video-sink", &bin.upcast::<gstreamer::Element>()).unwrap();

    let renderer = Renderer {
        xsize,
        ysize
    };

    let p = player.clone();
    appsink.set_callbacks(
        gstreamer_app::AppSinkCallbacks::new()
            .new_sample({
                move |sink| {
                    let sample = match sink.pull_sample() {
                        Some(sample) => sample,
                        None => return gstreamer::FlowReturn::Eos,
                    };

                    renderer.send_frame(&*sample).unwrap();

                    if autoplay == false {
                        // Just render first frame to get a static image
                        p.set_state(gstreamer::State::Paused).into_result().unwrap();

                    }

                    gstreamer::FlowReturn::Ok
                }
            })
            .eos({
                move |_| {
                    std::process::exit(0);
                }
            })
            .build()
    );

    if mute == true {
        player.set_property("volume", &0.0).unwrap();
    }

    player.set_state(gstreamer::State::Playing).into_result().unwrap();


    let seek_time = gstreamer::ClockTime::from_seconds(5);

    for key in std::io::stdin().keys() {
        match key {
            Ok(Key::Char('q')) => break,
            Ok(Key::Char('>')) => {
                if let Some(mut time) = player.query_position::<gstreamer::ClockTime>() {
                    time += seek_time;

                    player.seek_simple(
                        gstreamer::SeekFlags::FLUSH,
                        gstreamer::format::GenericFormattedValue::from_time(time)
                    ).unwrap();
                }
            },
            Ok(Key::Char('<')) => {
                if let Some(mut time) = player.query_position::<gstreamer::ClockTime>() {
                    if time >= seek_time {
                        time -= seek_time;
                    } else {
                        time = gstreamer::ClockTime(Some(0));
                    }

                    player.seek_simple(
                        gstreamer::SeekFlags::FLUSH,
                        gstreamer::format::GenericFormattedValue::from_time(time)
                    ).unwrap();
                }
            }
            Ok(Key::Char('p')) => {
                player.set_state(gstreamer::State::Playing).into_result().unwrap();
            }
            Ok(Key::Char('a')) => {
                player.set_state(gstreamer::State::Paused).into_result().unwrap();
            }
            Ok(Key::Char('m')) => {
                player.set_property("volume", &0.0).unwrap();
            }
            Ok(Key::Char('u')) => {
                player.set_property("volume", &1.0).unwrap();
            }


            _ => {}
        }
    }
}




struct Renderer {
    xsize: usize,
    ysize: usize
}

impl Renderer {
    fn send_image(&self, image: DynamicImage) -> MResult<()> {
        let rendered_img = self.render_image(image);

        for line in rendered_img {
            write!(std::io::stdout(), "{}\n", line).unwrap();
        }

        Ok(())
    }


    fn send_frame(&self, frame: &gstreamer::sample::SampleRef) -> MResult<()> {
        let buffer = frame.get_buffer().unwrap();
        let map = buffer.map_readable().unwrap();
        let img = image::load_from_memory_with_format(&map,
                                                      image::ImageFormat::PNM)
            .unwrap();
        let rendered_img = self.render_image(img);

        for line in rendered_img {
            write!(std::io::stdout(), "{}\n", line).unwrap();
        }

        // Empty line means end of frame
        write!(std::io::stdout(), "\n").unwrap();

        Ok(())
    }

    pub fn render_image(&self, image: DynamicImage) -> Vec<String> {
        let (xsize, ysize) = self.max_size(&image);

        let img = image.resize_exact(xsize as u32,
                                     ysize as u32,
                                     FilterType::Nearest).to_rgba();


        let rows = img.pixels()
            .collect::<Vec<_>>()
            .chunks(xsize as usize)
            .map(|line| line.to_vec())
            .collect::<Vec<Vec<_>>>();

        rows.par_chunks(2)
            .map(|rows| {
                rows[0]
                    .par_iter()
                    .zip(rows[1].par_iter())
                    .map(|(upper, lower)| {
                        let upper_color = upper.to_rgb();
                        let lower_color = lower.to_rgb();

                        format!("{}{}▀{}",
                                Fg(Rgb(upper_color[0], upper_color[1], upper_color[2])),
                                Bg(Rgb(lower_color[0], lower_color[1], lower_color[2])),
                                termion::style::Reset
                        )
                    }).collect()
            }).collect()
    }

    pub fn max_size(&self, image: &DynamicImage) -> (usize, usize)
    {
        let xsize = self.xsize;
        let img_xsize = image.width();
        let img_ysize = image.height();
        let img_ratio = img_xsize as f32 / img_ysize as f32;

        let mut new_y = if img_ratio < 1 as f32 {
            xsize as f32 * img_ratio
        } else {
            xsize as f32 / img_ratio
        };
        if new_y as u32 % 2 == 1 {
            new_y += 1 as f32;
        }
        (xsize, new_y as usize)
    }
}
