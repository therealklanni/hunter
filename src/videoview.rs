use lazy_static;


use crate::widget::{Widget, WidgetCore};
use crate::fail::{HResult, HError, ErrorLog};
use crate::imgview::ImgView;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock,
                mpsc::{channel, Sender}};

use std::io::{BufRead, BufReader, Write};

impl std::cmp::PartialEq for VideoView {
    fn eq(&self, other: &Self) -> bool {
        self.core == other.core
    }
}

lazy_static! {
    static ref MUTE: Arc<RwLock<bool>> = Arc::new(RwLock::new(false));
    static ref AUTOPLAY: Arc<RwLock<bool>> = Arc::new(RwLock::new(true));
}

pub struct VideoView {
    core: WidgetCore,
    imgview: Arc<Mutex<ImgView>>,
    file: PathBuf,
    controller: Sender<String>,
    paused: bool,
    dropped: Arc<Mutex<bool>>,
    preview_runner: Option<Box<dyn FnOnce(bool, bool)
                                          -> HResult<()> + Send + 'static>>
}

impl VideoView {
    pub fn new_from_file(core: WidgetCore, file: &Path) -> VideoView {
        let (xsize, ysize) = core.coordinates.size_u();
        let (tx_cmd, rx_cmd) = channel();

        let imgview = ImgView {
            core: core.clone(),
            buffer: vec![],
            file: file.to_path_buf()
        };

        let imgview = Arc::new(Mutex::new(imgview));
        let thread_imgview = imgview.clone();

        let path = file.to_string_lossy().to_string();
        let sender = core.get_sender();
        let dropped = Arc::new(Mutex::new(false));
        let drop = dropped.clone();


        let run_preview = Box::new(move |auto, mute| -> HResult<()> {
            loop {
                if *drop.lock()? == true {
                    return Ok(());
                }

                let mut previewer = std::process::Command::new("preview-gen")
                    .arg(format!("{}", (xsize)))
                    .arg(format!("{}", (ysize+1)))
                    .arg(format!("{}", 1))
                    .arg(format!("{}", auto))
                    .arg(format!("{}", mute))
                    .arg(&path)
                    .stdin(std::process::Stdio::piped())
                    .stdout(std::process::Stdio::piped())
                    .stderr(std::process::Stdio::inherit())
                    .spawn()?;

                let mut stdout = BufReader::new(previewer.stdout.take()?);


                let mut frame = vec![];
                let newline = String::from("\n");
                let mut line_buf = String::new();

                loop {
                    //kill quickly after drop
                    if let Ok(cmd) = rx_cmd.try_recv() {
                        if cmd == "q" {
                            previewer.kill()
                                .map_err(|e| HError::from(e))
                                .log();
                            // Oh no, zombies!!
                            previewer.wait()
                                .map_err(|e| HError::from(e))
                                .log();;

                            return Ok(());
                        } else {
                            previewer.stdin.as_mut().map(|stdin| {
                                write!(stdin, "{}", cmd)
                                    .map_err(|e| HError::from(e))
                                    .log();
                                write!(stdin, "\n")
                                    .map_err(|e| HError::from(e))
                                    .log();;
                                stdin.flush()
                                    .map_err(|e| HError::from(e))
                                    .log();;
                            });
                        }
                    }


                    // Check if preview-gen finished and break out of loop to restart
                    if let Ok(Some(code)) = previewer.try_wait() {
                        if code.success() {
                            break;
                        } else { return Ok(()); }
                    }


                    let _line = stdout.read_line(&mut line_buf)?;

                    // Newline means frame is complete
                    if line_buf == newline {
                        if let Ok(mut imgview) = thread_imgview.lock() {
                            imgview.set_image_data(frame);
                            sender.send(crate::widget::Events::WidgetReady)
                                .map_err(|e| HError::from(e))
                                .log();;
                        }

                        frame = vec![];
                        continue;
                    }

                    if line_buf != newline {
                        frame.push(line_buf);
                        line_buf = String::new();
                    }
                }
            }
        });



        VideoView {
            core: core.clone(),
            imgview: imgview,
            file: file.to_path_buf(),
            controller: tx_cmd,
            paused: false,
            dropped: dropped,
            preview_runner: Some(run_preview)
        }
    }

    pub fn start_video(&mut self) -> HResult<()> {
        let runner = self.preview_runner.take();
        let dropper = self.dropped.clone();
        let autoplay = self.autoplay();
        let mute = self.mute();

        if runner.is_some() {
            self.clear().log();
            std::thread::spawn(move || {
                let sleeptime = std::time::Duration::from_millis(50);
                let mut run = true;
                std::thread::sleep(sleeptime);
                dropper.lock().map(|dropper| {
                    if *dropper == false {
                        run = true;
                    }
                }).map_err(|e| HError::from(e)).log();
                if run == true {
                    runner.map(|runner| runner(autoplay, mute));
                }
            });
        }
        Ok(())
    }

    pub fn play(&self) -> HResult<()> {
        Ok(self.controller.send(String::from("p"))?)
    }

    pub fn pause(&self) -> HResult<()> {
        Ok(self.controller.send(String::from ("a"))?)
    }

    pub fn toggle_pause(&mut self) -> HResult<()> {
        if self.paused {
            self.play()?;
            self.toggle_autoplay();
            self.paused = false;
        }
        else {
            self.pause()?;
            self.toggle_autoplay();
            self.paused = true;
        }
        Ok(())
    }

    pub fn quit(&self) -> HResult<()> {
        Ok(self.controller.send(String::from("q"))?)
    }

    pub fn seek_forward(&self) -> HResult<()> {
        Ok(self.controller.send(String::from(">"))?)
    }

    pub fn seek_backward(&self) -> HResult<()> {
        Ok(self.controller.send(String::from("<"))?)
    }

    pub fn autoplay(&self) -> bool {
        if let Ok(autoplay) = AUTOPLAY.read() {
            return *autoplay;
        }
        return true;
    }

    pub fn mute(&self) -> bool {
        if let Ok(mute) = MUTE.read() {
            return *mute;
        }
        return false;
    }

    pub fn toggle_autoplay(&self) {
        if let Ok(mut autoplay) = AUTOPLAY.write() {
            *autoplay = dbg!(!*autoplay);
        }
    }

    pub fn toggle_mute(&self) {
        if let Ok(mut mute) = MUTE.write() {
            *mute = dbg!(!*mute);
            if *mute {
                self.controller.send(String::from("m")).ok();
            } else {
                self.controller.send(String::from("u")).ok();
            }
        }
    }
}

impl Widget for VideoView {
    fn get_core(&self) -> HResult<&WidgetCore> {
        Ok(&self.core)
    }

    fn get_core_mut(&mut self) -> HResult<&mut WidgetCore> {
        Ok(&mut self.core)
    }

    fn refresh(&mut self) -> HResult<()> {
        self.start_video().log();
        Ok(())
    }

    fn get_drawlist(&self) -> HResult<String> {
        self.imgview.lock()?.get_drawlist()
    }
}

impl Drop for VideoView {
    fn drop(&mut self) {
        dbg!("dropped");
        self.dropped.lock().map(|mut dropper| {
            *dropper = true;
            self.controller.send(String::from("q")).ok();
        }).map_err(|e| {
            self.controller.send(String::from("q")).ok();
            HError::from(e)
        }).log();

        self.clear().log();
    }
}
