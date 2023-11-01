use std::cmp::max;
use std::fmt::{Display, Formatter};
use std::fs::File;
use std::io::prelude::*;
use std::time;

use chrono::{naive::NaiveTime, Timelike};
use clap::Parser;
use serde::Deserialize;

const DEFAULT_DURATION_RASTER: u32 = 30;
const DEFAULT_COURSE_TYPE: &str = "ride";
const DEFAULT_AUTHOR: &str = "Mathias Lieber";
const DEFAULT_TIME_MODE: &str = "time";

/// CLI options
#[derive(Parser)]
struct CourseBuilder {
    /// course name
    name: String,
    /// optional description
    #[arg(short, long)]
    description: Option<String>,
    /// customizable author
    #[arg(long, short = 'A', default_value_t = DEFAULT_AUTHOR.to_string())]
    author: String,
    /// time mode: Must be "time" or "duration"
    #[arg(short, long, default_value_t = DEFAULT_TIME_MODE.to_string())]
    time_mode: String,
    /// customizable sport type
    #[arg(short = 'T', long, default_value_t = DEFAULT_COURSE_TYPE.to_string())]
    sport_type: String,
    /// absolute FTP in watts
    ftp: u16,
    /// time shrink factor
    #[arg(short, long, default_value_t = 1.0)]
    acceleration: f64,
    /// power scale factor
    #[arg(short, long, default_value_t = 1.0)]
    scale: f64,
    /// duration rasterization in seconds
    #[arg(short, long, default_value_t = DEFAULT_DURATION_RASTER)]
    raster: u32,
    /// path to the CSV file to read
    file: std::path::PathBuf,
}

#[derive(Debug, Deserialize)]
struct Record {
    time: String,
    #[serde(deserialize_with = "csv::invalid_option")]
    power: Option<u16>,
    text: Option<String>,
}

#[derive(Debug)]
struct Step {
    time: NaiveTime,
    watts: Option<u16>,
    text: Option<String>,
}

struct Course {
    name: String,
    description: Option<String>,
    author: String,
    sport_type: String,
    sections: Vec<Section>,
}

#[derive(Debug)]
struct Section {
    start: u32,
    duration: u32,
    power: f64,
    text: Vec<Hint>,
}

#[derive(Debug)]
struct Hint {
    offset: u32,
    text: String,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let builder = CourseBuilder::parse();

    match &builder.time_mode as &str {
        "time" | "duration" => {},
        _ => panic!("Error: time mode must be \"time\" or \"duration\".")
    }

    builder.run()
}

impl CourseBuilder {
    fn run(&self) -> Result<(), Box<dyn std::error::Error>> {
        let records = self.read_csv_file()?;

        let steps = self.parse_records(&records)?;

        let course = Course{
            name: self.name.clone(),
            description: self.description.clone(),
            author: self.author.clone(),
            sport_type: self.sport_type.clone(),
            sections: self.translate(steps),
        };

        // Let's just write to stdout (and expand tabs)
        println!("{}", course.to_string().replace("\t", "    "));
        Ok(())
    }

    fn read_csv_file(&self) -> std::io::Result<Vec<Record>> {
        let mut file = File::open(&*self.file.to_string_lossy())?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;

        // TODO: Parse file entries
        let mut reader = csv::Reader::from_reader((&contents as &str).as_bytes());

        let mut line = 0;
        let mut records: Vec<Record> = Vec::new();
        for record in reader.deserialize() {
            line += 1;
            match record {
                Err(err) => {
                    let msg = format!("Error in line {}: {}", line, err.to_string());
                    return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, msg));
                },
                Ok(record) => {
                    // println!("{:?}", record);
                    records.push(record);
                }
            }
        }

        Ok(records)
    }

    fn parse_records(&self, records: &Vec<Record>) -> Result<Vec<Step>, Box<dyn std::error::Error>> {
        let mut line = 0;

        let mut steps = Vec::new();
        let mut last_time = NaiveTime::from_hms_opt(0, 0, 0).unwrap();

        for result in records {
            line += 1;

            let mut step: Step = self.parse_step(&result)?;

            // TODO: Translate durations to time
            if self.time_mode.eq("duration") {
                let new_end = last_time + time::Duration::new(step.time.num_seconds_from_midnight() as u64, 0);

                step.time = last_time;
                last_time = new_end;
            } else {
                // check if time is monotonic ascending
                if step.time < last_time {
                    let msg = format!("Error in line {}: time {} is before last time {}", line, step.time.to_string(), last_time.to_string());
                    return Err(Box::<dyn std::error::Error>::from(msg));
                }
            }

            steps.push(step);
        }

        return Ok(steps)
    }

    fn parse_step(&self, record: &Record) -> Result<Step, Box<dyn std::error::Error>> {
        let time = NaiveTime::parse_from_str(&record.time, "%H:%M:%S")?;
        let step = Step{time, watts: record.power, text: record.text.clone()};
        Ok(step)
    }

    fn translate(&self, steps: Vec<Step>) -> Vec<Section> {
        let mut out = Vec::<Section>::new();
        let mut cur_sec: Option<Section> = None;

        for step in steps {
            // Use acceleration factor
            let local_time = (step.time.num_seconds_from_midnight() as f64 / self.acceleration).round() as u32;

            let power = match step.watts {
                // Scale power
                Some(watts) => Some(((watts as f64 * self.scale / self.ftp as f64 * 100.0).round() as u32) as f64 / 100.0),
                None => None,
            };

            let mut offset = 0;
            let mut rounded_offset = 0;
            if let Some(sec) = &mut cur_sec {
                if local_time > sec.start {
                    offset = local_time - sec.start;
                    rounded_offset = round(offset, self.raster);
                    sec.duration = rounded_offset;
                }
            }

            match (&mut cur_sec, power, &step.text) {
                (Some(sec), None, Some(text)) => {
                    // add text to existing node
                    rounded_offset = round(offset, 5);
                    sec.text.push(Hint {offset: rounded_offset, text: text.clone()});
                    if sec.duration < rounded_offset {
                        sec.duration += self.raster;
                    }
                },
                (section, Some(power), _) => {
                    let mut new_start_time = round(local_time, self.raster);
                    // close existing node
                    if let Some(sec) = &section {
                        new_start_time = sec.start + sec.duration;
                        out.push(cur_sec.unwrap());
                    }

                    // start new node
                    let mut sec = Section{ start: new_start_time, duration: self.raster, power, text: vec!()};
                    if let Some(text) = &step.text {
                        sec.text.push(Hint{offset: 0, text: text.clone()})
                    }
                    cur_sec = Some(sec);
                },
                _ => {},
            }
        }

        if let Some(sec) = cur_sec {
            out.push(sec);
        }

        out
    }
}

impl Display for Course {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        write!(f, "<workout_file>\n")?;

        self.write_header(f)?;

        write!(f, "</workout_file>\n")?;
        Ok(())
    }
}

impl Course {
    fn write_header(&self, f: &mut Formatter) -> std::fmt::Result {
        write!(f, "\t<author>{}</author>\n", self.author)?;
        write!(f, "\t<name>{}</name>\n", self.name)?;

        if let Some(description) = &self.description {
            write!(f, "\t<description>{}</description>\n", description)?;
        } else {
            write!(f, "\t<description/>\n")?;
        }

        write!(f, "\t<sportType>{}</sportType>\n", self.sport_type)?;
        write!(f, "\t<tags/>\n")?;

        self.write_sections(f)?;

        Ok(())
    }

    fn write_sections(&self, f: &mut Formatter) -> std::fmt::Result {
        write!(f, "\t<workout>\n")?;

        for sec in &self.sections {
            if sec.text.len() == 0 {
                write!(f, "\t\t<SteadyState Duration=\"{}\" Power=\"{}\" pace=\"0\"/>\n", sec.duration, sec.power)?;
            } else {
                write!(f, "\t\t<SteadyState Duration=\"{}\" Power=\"{}\" pace=\"0\">\n", sec.duration, sec.power)?;
                for hint in &sec.text {
                    write!(f, "\t\t\t<textevent timeoffset=\"{}\" message=\"{}\"/>\n", hint.offset, hint.text)?;
                }
                write!(f, "\t\t</SteadyState>\n")?;
            }
        }

        write!(f, "\t</workout>\n")?;
        Ok(())
    }
}

fn round(offset: u32, step: u32) -> u32 {
    max(step, ((offset as f64 / step as f64).round()) as u32 * step)
}

#[test]
fn test_round() {
    assert_eq!(30, round(0, 30));
    assert_eq!(30, round(10,30));
    assert_eq!(10, round(12,5));
    assert_eq!(30, round(20, 30));
    assert_eq!(30, round(30, 30));
    assert_eq!(30, round(40, 30));
    assert_eq!(60, round(50, 30));
}
