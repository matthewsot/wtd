use std::fs::File;
use std::io::prelude::*;
use std::path::Path;
use std::str::FromStr;
use std::cmp::Ordering;
use std::collections::HashSet;
use std::collections::HashMap;
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
    let formats = vec!["%l:%M%p", "%H:%M"];
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
        } else {
            if t.details.len() > 0 {
                t.details.push(' ');
            }
            t.details.push_str(tok.trim());
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

fn does_overlap(timespan_start: &NaiveTime, timespan_end: &NaiveTime, task: &Task) -> bool {
    return match [task.start_time, task.end_time] {
        [Some(start), Some(end)] => (&start <= timespan_start && timespan_start < &end)
                                    || (&start < timespan_end && timespan_end < &end),
        _ => false,
    }
}

fn tasks_to_html(tasks: &Vec<Task>) -> String {
    let public_tags = HashMap::from([
        ("busy", "I will be genuinely busy, e.g., a meeting with others."),
        ("rough", "The nature of the event (e.g., a hike) makes it difficult to preduct the exact start/end times."),
        ("tentative", "This event timing is only tentative."),
        ("join-me", "This is an open event; if you're interested in attending with me please reach out!"),
        ("self", "This is scheduled time for me to complete a specific work or personal task; I can usually reschedule such blocks when requested."),
    ]);

    let mut html = "<html><head><meta charset=\"UTF-8\"><title>Calendar</title><link rel=\"stylesheet\" href=\"stylesheet.css\"></link></head><body>".to_string();

    let n_days = 14;
    let start_period = Local::now().date().naive_local();
    let end_period = start_period + Duration::days(n_days);
    let mut week_task_ids: Vec<usize> = Vec::new();
    for (i, task) in tasks.iter().enumerate() {
        if task.date >= start_period && task.date < end_period {
            week_task_ids.push(i);
        }
    }

    let min_incr: i64 = 15;
    let timespans_per_day = (24 * 60 ) / min_incr;
    let mut table: Vec<Vec<Option<usize>>> = Vec::new();
    let mut table_tags: Vec<Vec<Vec<&String>>> = Vec::new();
    for i in 0..timespans_per_day {
        table.push(Vec::new());
        table_tags.push(Vec::new());
        for _ in 0..n_days {
            table[i as usize].push(None);
            table_tags[i as usize].push(vec![]);
        }
    }

    html.push_str("<table>");
    html.push_str("<tr><th>Time</th>");
    for offset in 0..n_days {
        html.push_str("<th>");
        html.push_str(&(start_period + Duration::days(offset)).format("%a %-m/%-d/%y").to_string());
        html.push_str("</th>");
    }
    html.push_str("</tr>");

    week_task_ids.sort_by(|a, b| cmp_tasks(&tasks[*a], &tasks[*b]));

    for i in 0..timespans_per_day {
        let timespan_start = NaiveTime::from_hms(0, 0, 0) + Duration::minutes(i * min_incr);
        let timespan_end = NaiveTime::from_hms(0, 0, 0) + Duration::minutes((i + 1) * min_incr);
        for offset in 0..n_days {
            // (1) Find all task ids that intersect this timespan on this day.
            let this_date = start_period + Duration::days(offset);
            let on_this_date: Vec<usize>
                = week_task_ids.iter().map(|&idx| idx)
                  .filter(|&idx| tasks[idx].date == this_date).collect();
            let intersecting: Vec<usize>
                = on_this_date.iter().map(|&idx| idx)
                  .filter(|&idx| does_overlap(&timespan_start, &timespan_end, &tasks[idx])).collect();
            // (2) Find the event ending first and place it in the table.
            table[i as usize][offset as usize] = intersecting.iter()
                .map(|&idx| idx)
                .min_by_key(|&idx| tasks[idx].end_time.expect("Should have an end time at this point..."));
            // (3) Collect all the (public) tags used.
            let mut span_public_tags: HashSet<&String> = HashSet::new();
            for idx in intersecting {
                span_public_tags.extend(tasks[idx].tags.iter().map(|t| t));
            }
            for tag in span_public_tags {
                if public_tags.contains_key(tag.as_str()) {
                    table_tags[i as usize][offset as usize].push(tag);
                }
            }
            table_tags[i as usize][offset as usize].sort();
        }
    }
    for row_idx in 0..timespans_per_day {
        let timespan_start = NaiveTime::from_hms(0, 0, 0) + Duration::minutes(row_idx * min_incr);
        html.push_str("<tr><td><b>");
        html.push_str(&timespan_start.format("%l:%M %p").to_string());
        html.push_str("</b></td>");
        for col_idx in 0..n_days {
            let task_idx = table[row_idx as usize][col_idx as usize];
            let all_tags = &table_tags[row_idx as usize][col_idx as usize];
            match task_idx {
                Some(idx) => {
                    if row_idx == 0 || table[(row_idx - 1) as usize][col_idx as usize] != task_idx {
                        let mut rowspan = 0;
                        for i in rowspan..timespans_per_day {
                            if table[i as usize][col_idx as usize] == task_idx {
                                rowspan += 1;
                            }
                        }
                        html.push_str("<td class=\"has-task");
                        for tag in all_tags {
                            html.push_str(" tag-");
                            html.push_str(tag.as_str());
                        }
                        html.push_str("\" rowspan=\"");
                        html.push_str(rowspan.to_string().as_str());
                        html.push_str("\">");
                        html.push_str("<a href=\"#");
                        html.push_str("task-");
                        html.push_str(idx.to_string().as_str());
                        html.push_str("\">");
                        if all_tags.len() == 0 {
                            html.push_str("has-task");
                        }
                        let mut any_yet = false;
                        for tag in all_tags {
                            if any_yet {
                                html.push_str(", ");
                            }
                            html.push_str(tag.as_str());
                            any_yet = true;
                        }
                        html.push_str("</a></td>");
                    }
                },
                _ => {
                    html.push_str("<td></td>");
                },
            }
        }
        html.push_str("</tr>");
    }
    html.push_str("</table><ul>");
    for i in week_task_ids.iter() {
        let task = &tasks[*i];
        html.push_str("<li id=\"task-");
        html.push_str(i.to_string().as_str());
        html.push_str("\">");
        html.push_str(task.date.format("%a %-m/%-d/%y ").to_string().as_str());
        match [task.start_time, task.end_time] {
            [Some(start), Some(end)] => {
                html.push_str(start.format("%l:%M%p").to_string().as_str());
                html.push_str(" -- ");
                html.push_str(end.format("%l:%M%p").to_string().as_str());
            }
            _ => (),
        }
        html.push_str("<ul>");
        if task.tags.contains(&"public".to_string()) {
            html.push_str("<li><b>Description:</b> ");
            html.push_str(task.details.as_str());
            html.push_str("</li>");
        }
        for tag in &task.tags {
            if public_tags.contains_key(&tag.as_str()) {
                html.push_str("<li>Tagged <b>");
                html.push_str(tag.as_str());
                html.push_str(":</b> ");
                html.push_str(public_tags.get(&tag.as_str()).expect(""));
                html.push_str("</li>");
            }
        }
        html.push_str("</ul>");
        html.push_str("</li>");
    }
    html.push_str("</ul></body></html>");
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
