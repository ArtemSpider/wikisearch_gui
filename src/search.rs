use std::collections::HashMap;
use std::collections::LinkedList;
use std::mem::swap;
use std::str::from_utf8;
use std::sync::mpsc::{self, Sender};
use std::sync::mpsc::TryRecvError;
use std::thread;

use reqwest::blocking::Client;

fn get_html(from: &str, client: &mut Client) -> Result<String, Box<dyn std::error::Error>> {
    Ok(client.get(from).send()?.text()?)
}

fn get_links(from: &str, client: &mut Client) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    //let start = Instant::now();

    let html = get_html(from, client)?;

    //println!("get_html took {:?}", start.elapsed());

    let beg = html.find("<div id=\"mw-content-text\"");
    if let Some(beg) = beg {
        let mut res = Vec::new();

        let mut x = from_utf8(&html.as_bytes()[beg..]).unwrap();
        
        while let Some(ind) = x.find("<a href=\"/wiki/") {
            x = &x[ind + 15..];
            let ref_end = x.find("\"").unwrap();
            let r = &x[..ref_end];
            if !r.starts_with("File:") && 
                !r.starts_with("Category:") && 
                !r.starts_with("Special:") && 
                !r.starts_with("Talk:") && 
                !r.starts_with("Wikipedia:") && 
                !r.starts_with("Template:") && 
                !r.starts_with("Portal:") && 
                !r.starts_with("Help:") {
                res.push("https://en.wikipedia.org/wiki/".to_string() + r);
            }
            x = &x[ref_end..];
        }

        Ok(res)
    }
    else {
        Ok(vec![])
    }
}

#[derive(PartialEq, Eq)]
enum ThreadState {
    Idle,
    Processing,
    Error,
}

pub fn search(from: &str, to: &str, num_of_threads: usize, max_num_of_links: usize, num_of_links_sender: Sender<(usize, usize)>, dead_threads_sender: Sender<usize>) -> Vec<String> {
    if from == to {
        return vec![from.to_string()];
    }

    let mut all = HashMap::new();
    all.insert(from.to_string(), "".to_string());

    let mut in_search = LinkedList::new();
    in_search.push_back(from.to_string());

    let mut in_search_next = LinkedList::new();

    let mut txs = Vec::new();
    let mut rxs = Vec::new();
    let mut handlers = Vec::new();
    let mut states = Vec::new();
    let mut plinks: Vec<Option<String>> = Vec::new();

    for _ in 0..num_of_threads {
        let (tx1, rx) = mpsc::channel(); // from main thread
        let (tx, rx1) = mpsc::channel(); // to main thread
        txs.push(tx1);
        rxs.push(rx1);

        handlers.push(thread::spawn(move || {
            let mut client = Client::default();
            
            loop {
                let url = rx.recv();
                if url.is_err() {
                    break;
                }
                let url: String = url.unwrap();
                if tx.send(get_links(&url[..], &mut client).unwrap()).is_err() {
                    break;
                }
            }
        }));

        states.push(ThreadState::Idle);
        plinks.push(None);
    }

    let mut processed = 0usize;

    //let gstart = Instant::now();

    let mut _level = 0usize;

    let mut num_of_links_changed = true;
    // while path betweeen links is not found
    loop {
        // while every link is in_search is not processed
        while !in_search.is_empty() || states.contains(&ThreadState::Processing) {
            if num_of_links_changed {
                let res = num_of_links_sender.send((processed, in_search.len() + in_search_next.len()));
                if res.is_err() {
                    eprintln!("Main thread is closed");
                    return vec![];
                }

                num_of_links_changed = false;
            }

            if max_num_of_links > 0 && in_search.len() + in_search_next.len() >= max_num_of_links {
                eprintln!("Max number of links in the queue exceeded");
                return vec![];
            }

            for i in 0..num_of_threads {
                if states[i] == ThreadState::Processing {
                    let r = rxs[i].try_recv();

                    match r {
                        Ok(v) => {
                            processed += 1;
                            num_of_links_changed = true;

                            for c in &v {
                                if c == to {
                                    let mut res = vec![];
                    
                                    let mut li = c.clone();
                                    res.push(li.clone());
                                    li = plinks[i].clone().unwrap();
                                    res.push(li.clone());
                    
                                    while li != from {
                                        li = all.get(&li).unwrap().clone();
                                        res.push(li.clone());
                                    }
                                    res.reverse();
                                    return res;
                                }
                                
                                if !all.contains_key(c) {
                                    all.insert(c.clone(), plinks[i].clone().unwrap());
                                    in_search_next.push_back(c.clone());
                                    num_of_links_changed = true;
                                }
                            }

                            states[i] = ThreadState::Idle;
                            plinks[i] = None;
                        },
                        Err(e) => {
                            if e == TryRecvError::Disconnected {
                                states[i] = ThreadState::Error;
                                in_search.push_front(plinks[i].clone().unwrap());
                                num_of_links_changed = true;
                                plinks[i] = None;

                                eprintln!("Thread {} died", i);
                                dead_threads_sender.send(i).unwrap();
                            }
                        },
                    }
                }
            }

            for i in 0..num_of_threads {
                if states[i] == ThreadState::Idle {
                    if !in_search.is_empty() {
                        let link = in_search.pop_front().unwrap();
                        num_of_links_changed = true;
                        if txs[i].send(link.clone()).is_err() {
                            states[i] = ThreadState::Error;
                            eprintln!("Error while sending to thread №{}", i);
                        }
                        states[i] = ThreadState::Processing;
                        plinks[i] = Some(link.clone());

                        //println!("List size is {}. Checking {}", in_search.len() + in_search_next.len(), link);
                    }
                }
            }
        }

        swap(&mut in_search, &mut in_search_next);
        _level += 1;
    }
}