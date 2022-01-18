use std::{thread, sync::mpsc::{Receiver, self, TryRecvError}, time::{Instant, Duration}};

use eframe::{egui, epi};

use crate::search;

pub struct SearchingInfo {
    search_from: String,
    search_to: String,

    //search_thread: JoinHandle<()>,
    result_reciever: Receiver<Vec<String>>,

    num_of_links: Receiver<(usize, usize)>, // (num_of_processed, num_in_queue)
    num_of_processed: usize,
    num_in_queue: usize,

    threads: usize,

    dead_threads_rec: Receiver<usize>,
    threads_state: Vec<bool>,

    start_instant: Instant,
}

impl SearchingInfo {
    pub fn new(from: &str, to: &str, threads: usize) -> Self {
        let (nol_sender, nol_reciever) = mpsc::channel(); // num_of_links
        let (dt_sender, dt_reciever) = mpsc::channel(); // dead_threads

        let search_from = from.to_string();
        let search_to = to.to_string();

        let sf = from.to_string();
        let st = to.to_string();

        let (res_sender, res_reciever) = mpsc::channel();

        let _thread = thread::spawn(move || {
            let res = search::search(sf.as_str(), st.as_str(), threads, 0, nol_sender, dt_sender);
            res_sender.send(res).unwrap();
        });
        
        Self {
            search_from,
            search_to,
            //search_thread: thread,
            result_reciever: res_reciever,
            num_of_links: nol_reciever,
            num_of_processed: 0,
            num_in_queue: 0,
            threads,
            dead_threads_rec: dt_reciever,
            threads_state: vec![true; threads],
            start_instant: Instant::now(),
        }
    }
}

pub struct FoundInfo {
    search_from: String,
    search_to: String,
    used_threads: usize,
    num_of_processed: usize,
    duration: Duration,

    path: Vec<String>,
}

impl FoundInfo {
    pub fn new(searching_info: &SearchingInfo, path: Vec<String>) -> Self {
        Self {
            search_from: searching_info.search_from.clone(),
            search_to: searching_info.search_to.clone(),
            used_threads: searching_info.threads,
            num_of_processed: searching_info.num_of_processed,
            duration: searching_info.start_instant.elapsed(),
            path,
        }
    }
}

pub enum State {
    Input,
    Searching(SearchingInfo),       // search thread, num of parsed links, num of links in queue, alive threads
    Found(FoundInfo),               // path from search_from to search_to
}

pub struct TemplateApp {
    state: State,
    search_from: String,
    search_to: String,
    threads: usize,
}

pub fn is_valid_wiki_link(url: &str) -> bool {
    if !url.starts_with("https://en.wikipedia.org/wiki/") {
        return false;
    }
    reqwest::blocking::get(url).is_ok()
}

impl Default for TemplateApp {
    fn default() -> Self {
        Self {
            state: State::Input,
            search_from: "https://en.wikipedia.org/wiki/It_Is_the_Law".to_string(),
            //search_to: "https://en.wikipedia.org/wiki/Yale_University".to_string(),
            //search_from: "https://en.wikipedia.org/wiki/Dave_Hollins".to_string(),
            search_to: "https://en.wikipedia.org/wiki/Dab_(dance)".to_string(),
            threads: 1,
        }
    }
}

impl TemplateApp {
    fn input_state(&mut self, _: &egui::CtxRef, ui: &mut egui::Ui) {
        egui::Grid::new("1").max_col_width(10000f32).show(ui, |ui| {
            ui.label("From: ");
            ui.add_enabled(true, egui::TextEdit::singleline(&mut self.search_from));
            ui.end_row();

            ui.label("To: ");
            ui.add_enabled(true, egui::TextEdit::singleline(&mut self.search_to));
            ui.end_row();
        });
        ui.add(egui::Slider::new(&mut self.threads, 1..=100).text("threads"));

        if ui.button("Search").clicked() {
            if self.threads > 0 && self.threads <= 100 && 
                is_valid_wiki_link(self.search_from.as_str()) && is_valid_wiki_link(self.search_to.as_str()) {
                self.state = State::Searching(SearchingInfo::new(self.search_from.as_str(), self.search_to.as_str(), self.threads));
            }
        }
    }

    fn searching_state(&mut self, ctx: &egui::CtxRef, ui: &mut egui::Ui) {
        if let State::Searching(info) = &mut self.state {
            let res_res = info.result_reciever.try_recv();
            match res_res {
                Ok(res) => {
                    // search thread is joined by now
                    self.state = State::Found(FoundInfo::new(info, res));
                    return;
                },
                Err(reason) => {
                    match reason {
                        TryRecvError::Empty => {},
                        TryRecvError::Disconnected => {
                            panic!("Search thread is dead");
                        },
                    }
                },
            }

            ctx.request_repaint();

            egui::Grid::new("1").max_col_width(10000f32).show(ui, |ui| {
                ui.label("From: ");
                ui.add_enabled(false, egui::TextEdit::singleline(&mut info.search_from));
                ui.end_row();
    
                ui.label("To: ");
                ui.add_enabled(false, egui::TextEdit::singleline(&mut info.search_to));
                ui.end_row();
            });
            ui.label(format!("{} thread{} used", info.threads.to_string(), if info.threads > 1 {"s are"} else {" is"}));
    
            loop {
                let nol_res = info.num_of_links.try_recv();
                match nol_res {
                    Ok((pl, ql)) => {
                        info.num_of_processed = pl;
                        info.num_in_queue = ql;
                    },
                    Err(reason) => {
                        match reason {
                            TryRecvError::Empty => {},
                            TryRecvError::Disconnected => {
                                panic!("Search thread is dead");
                            },
                        }
                        break;
                    },
                }
            }
    
            ui.label(format!("Pages processed: {} ({} per second)", info.num_of_processed, (info.num_of_processed as f32 / info.start_instant.elapsed().as_secs_f32()) as u32));
            ui.label(format!("Pages in queue: {}", info.num_in_queue));
            ui.label(format!("Elapsed time: {}s", info.start_instant.elapsed().as_secs_f32().to_string()));

            let dt_res = info.dead_threads_rec.try_recv();
            match dt_res {
                Ok(dead_thread_ind) => {
                    info.threads_state[dead_thread_ind] = !info.threads_state[dead_thread_ind];
                },
                Err(reason) => {
                    match reason {
                        TryRecvError::Empty => {},
                        TryRecvError::Disconnected => {
                            panic!("Search thread is dead");
                        },
                    }
                },
            }

            for (i, alive) in info.threads_state.iter().enumerate() {
                if !alive {
                    ui.label(format!("Thread {} is dead", i));
                }
            }
        }
    }
        
    fn found_state(&mut self, _: &egui::CtxRef, ui: &mut egui::Ui) {
        if let State::Found(info) = &mut self.state {
            egui::Grid::new("1").max_col_width(10000f32).show(ui, |ui| {
                ui.label("From: ");
                ui.add_enabled(false, egui::TextEdit::singleline(&mut info.search_from));
                ui.end_row();
    
                ui.label("To: ");
                ui.add_enabled(false, egui::TextEdit::singleline(&mut info.search_to));
                ui.end_row();
            });
            ui.label(format!("{} thread{} were used", info.used_threads.to_string(), if info.used_threads > 1 {"s"} else {""}));

            ui.label(format!("Pages processed: {} ({} per second)", info.num_of_processed, (info.num_of_processed as f32 / info.duration.as_secs_f32()) as u32));
            ui.label(format!("Elapsed time: {}s", info.duration.as_secs_f32().to_string()));
    
            ui.label("Path:");
            for s in &info.path {
                ui.hyperlink(s);
            }
        }
    }
}

impl epi::App for TemplateApp {
    fn name(&self) -> &str {
        "wikisearch"
    }

    fn setup(
        &mut self,
        _ctx: &egui::CtxRef,
        _frame: &epi::Frame,
        _storage: Option<&dyn epi::Storage>,
    ) {}

    fn update(&mut self, ctx: &egui::CtxRef, _: &epi::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            match self.state {
                State::Input => self.input_state(ctx, ui),
                State::Searching(_) => self.searching_state(ctx, ui),
                State::Found(_) => self.found_state(ctx, ui),
            }
        });
    }
}
