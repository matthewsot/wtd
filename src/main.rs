use std::fs::File;
use std::io::prelude::*;
use std::path::Path;
use std::str::FromStr;
use std::convert::TryInto;
use std::cmp::Ordering;
use chrono::{Datelike, NaiveDate, NaiveTime, Weekday, Duration, Timelike, Local};
// use chrono::format::ParseError;

struct Task {
    date: NaiveDate,
    start_time: Option<NaiveTime>,
    end_time: Option<NaiveTime>,
    details: String,
    tags: Vec<String>,
}

fn parse_date_line(l: &str) -> Option<NaiveDate> {
    for maybe_date_str in l.split(' ') {
        match NaiveDate::parse_from_str(maybe_date_str, "%m/%d/%y") {
            Err(_) => continue,
            Ok(date) => { return Some(date); }
        }
    };
    return None;
}

fn parse_day_line(l: &str) -> Weekday {
    let daystr = l.get(3..).expect("Day-of-week line not long enough...");
    return Weekday::from_str(daystr).expect("Misparse day-of-week str...");
}

fn parse_time(s_: &str) -> NaiveTime {
    let formats = vec!["%H:%M%p", "%H:%M"];
    let mut s = s_.to_string();
    if !s.contains(":") {
        if s.ends_with("M") {
            s.insert_str(s.len() - 2, ":00");
        } else {
            s.push_str(":00");
        }
    }
    for format in formats {
        match NaiveTime::parse_from_str(&s, format) {
            Err(_) => continue,
            Ok(parsed) => {
                if !format.contains("%p") && parsed.hour() < 6 {
                    return parsed + Duration::hours(12);
                }
                return parsed;
            }
        }
    }
    panic!("Couldn't parse time {}", s);
}

fn parse_duration(s: &str) -> chrono::Duration {
    // We try to find Mm, HhMm, Hh
    if s.contains("h") && s.contains("m") {
        // TODO: Decompose this case into the two below.
        let hstr = s.split("h").collect::<Vec<&str>>().get(0).expect("").to_string();
        let mstr = s.split("h").collect::<Vec<&str>>().get(1).expect("").split("m").collect::<Vec<&str>>().get(0).expect("Expected XhYm").to_string();
        let secs = ((hstr.parse::<u64>().unwrap() * 60)
                    + (mstr.parse::<u64>().unwrap())) * 60;
        return chrono::Duration::from_std(std::time::Duration::new(secs, 0)).unwrap();
    } else if s.contains("h") {
        let hstr = s.split("h").collect::<Vec<&str>>().get(0).expect("").to_string();
        let secs = hstr.parse::<u64>().unwrap() * 60 * 60;
        return chrono::Duration::from_std(std::time::Duration::new(secs, 0)).unwrap();
    } else if s.contains("m") {
        let hstr = s.split("m").collect::<Vec<&str>>().get(0).expect("").to_string();
        let secs = hstr.parse::<u64>().unwrap() * 60;
        return chrono::Duration::from_std(std::time::Duration::new(secs, 0)).unwrap();
    }
    panic!("Couldn't parse duration {}", s);
}

fn handle_task_details(l: &str, t: &mut Task) {
    if t.details.len() > 0 {
        t.details.push(' ');
    }
    t.details.push_str(l.trim());
    for tok in l.split(' ') {
        if tok.starts_with("+") {
            let tag = tok.get(1..).expect("Unexpected");
            t.tags.push(tag.to_string());
        } else if tok.starts_with("@") {
            let timestr = tok.get(1..).expect("Unexpected");
            if timestr.contains("+") { // @Start+Duration
                let parts: Vec<&str> = timestr.split("+").collect();
                match parts[..] {
                    [startstr, durstr] => {
                        t.start_time = Some(parse_time(startstr));
                        t.end_time = Some(t.start_time.unwrap() + parse_duration(durstr));
                    },
                    _ => panic!("Not 2 parts to {}\n", timestr)
                }
            } else if timestr.contains("--") { // @Start--End
                let parts: Vec<&str> = timestr.split("--").collect();
                match parts[..] {
                    [startstr, endstr] => {
                        t.start_time = Some(parse_time(startstr));
                        t.end_time = Some(parse_time(endstr));
                        if t.start_time > t.end_time {
                            panic!("Start time {} interpreted as after end time {}",
                                   startstr, endstr);
                        }
                    },
                    _ => panic!("Not 2 parts to {}\n", timestr)
                }
            } else {
                panic!("'{}' is not of the form Start+Duration or Start--End\n", timestr);
            }
        }
    }
}

fn cmp_tasks(a: &Task, b: &Task) -> Ordering {
    if a.date < b.date {
        return Ordering::Less;
    } else if b.date < a.date {
        return Ordering::Greater;
    }
    match [a.start_time, b.start_time] {
        [None, None] => return Ordering::Equal,
        [None, Some(_)] => return Ordering::Greater,
        [Some(_), None] => return Ordering::Less,
        [Some(atime), Some(btime)] => return if atime < btime { Ordering::Less } else { Ordering::Greater },
    }
}

fn tasks_to_html(tasks: &Vec<Task>) -> String {
    let mut html = "<html><head><title>Calendar</title><link rel=\"stylesheet\" href=\"stylesheet.css\"></link></head><body>".to_string();

    let today = Local::now().date().naive_local();
    let start_of_week = today - Duration::days(today.weekday().num_days_from_monday().try_into().unwrap());
    let start_of_next_week = today + (Duration::days(7) - Duration::days(today.weekday().num_days_from_monday().try_into().unwrap()));
    let mut week_task_ids: Vec<usize> = Vec::new();
    for (i, task) in tasks.iter().enumerate() {
        if task.date >= start_of_week && task.date < start_of_next_week {
            week_task_ids.push(i);
        }
    }

    html.push_str("<table><tr>");
    html.push_str("<th>Time</th>");
    for day_of_week in 0..7 {
        html.push_str("<th>");
        html.push_str(&(start_of_week + Duration::days(day_of_week)).format("%A %-m/%-d/%y").to_string());
        html.push_str("</th>");
    }
    html.push_str("</tr>");

    week_task_ids.sort_by(|a, b| cmp_tasks(&tasks[*a], &tasks[*b]));

    let min_incr = 15;
    let mut time = NaiveTime::from_hms(0, 0, 0);
    loop {
        html.push_str("<tr><td>");
        html.push_str(&time.format("%l:%M %p").to_string());
        html.push_str("</td>");
        for day_of_week in 0..7 {
            // TODO: Use a smarter data structure for this.
            let mut any_task = false;
            for i in week_task_ids.iter() {
                let task = &tasks[*i];
                let task_day = task.date.weekday().num_days_from_monday();
                if task_day > day_of_week {
                    break;
                } else if task_day < day_of_week {
                    continue;
                }
                match [task.start_time, task.end_time] {
                    [Some(start), Some(end)] => {
                        if time >= start && time < end {
                            any_task = true;
                            break;
                        }
                    }
                    _ => continue
                }
            }
            if any_task {
                html.push_str("<td class=\"busy\"></td>");
            } else {
                html.push_str("<td></td>");
            }
        }
        html.push_str("</tr>");

        time = time + Duration::minutes(min_incr);
        if time == NaiveTime::from_hms(0, 0, 0) {
            break;
        }
    }
    html.push_str("</table></body></html>");
    return html;
}

// https://doc.rust-lang.org/std/fs/struct.File.html
fn main() {
    let path = Path::new("wtd.md");
    let display = path.display();

    // Open the path in read-only mode, returns `io::Result<File>`
    let mut file = match File::open(&path) {
        Err(why) => panic!("Error opening {}: {}", display, why),
        Ok(file) => file,
    };

    // Read the file contents into a string, returns `io::Result<usize>`
    let mut s = String::new();
    match file.read_to_string(&mut s) {
        Err(why) => panic!("Couldn't read {}: {}", display, why),
        Ok(_) => {

            let mut tasks = Vec::new();
            let mut start_date = None;
            let mut the_date = None;
            for l in s.split('\n') {
                if l.starts_with("# ") {
                    // '# 12/27/21', starts a new week block
                    start_date = parse_date_line(l);
                } else if l.starts_with("## ") {
                    // '## Monday/Tuesday/...', starts a new day block
                    // Need to compute the actual date, basically looking for the first one after
                    // start_date.
                    let dayofweek = parse_day_line(l);
                    let mut current = start_date.expect("Invalid or missing '# ' date");
                    the_date = loop {
                        if current.weekday() == dayofweek {
                            break Some(current);
                        }
                        current = current.succ();
                    };
                } else if l.starts_with("- [ ]") {
                    // '- [ ] ...', starts a new task block
                    let date = the_date.expect("No current date parsed yet...");
                    tasks.push(Task {
                        date: date,
                        start_time: None,
                        end_time: None,
                        details: "".to_string(),
                        tags: Vec::new(),
                    });
                    let details = l.get(5..).expect("").trim();
                    handle_task_details(details, tasks.last_mut().expect("Unexpected error..."));
                } else if l.starts_with(" ") {
                    // Extends the last task.
                    handle_task_details(l, tasks.last_mut().expect("Unexpected error..."));
                } else {
                    if l.trim().len() > 0 {
                        print!("Ignoring line: {}\n", l);
                    }
                }
            }
            print!("{}\n", tasks_to_html(&tasks));
        }
    }
}
