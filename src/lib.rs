// This file is distributed under the BSD 3-clause license.  See file LICENSE.
// Copyright (c) 2022 Rex Kerr and Calico Life Sciences LLC


use std::fmt;
use std::fmt::Display;

use serde::{Serialize, Deserialize};

use average::Estimate;

pub mod parsing;

pub use parsing::*;


pub trait Entitled {
    fn push_subtitle(&self, specifier: &str, to: &mut String);
    fn push_title(&self, to: &mut String) { self.push_subtitle(&"", to); }

    fn title(&self) -> String {
        let mut s = String::new();
        self.push_title(&mut s);
        s
    }
}


pub fn the_area(input: &Vec<DataLine>) -> average::Variance {
    input.iter().map(|line| line.area).filter(|x| x.is_finite()).collect()
}

pub fn the_midline(input: &Vec<DataLine>) -> average::Variance {
    input.iter().map(|line| line.midline).filter(|x| x.is_finite()).collect()
}

fn median5(input: &[f64; 5]) -> f64 {
    let mut a = input[0];
    let mut b = input[1];
    if a > b { let temp = a; a = b; b = temp; }
    let mut c = input[2];
    let mut d = input[3];
    if c > d { let temp = c; c = d; d = temp; }
    if a < c { a = c; }
    if b > d { b = d; }
    if a > b { let temp = a; a = b; b = temp; }

    if input[4] <= a      { a }
    else if input[4] >= b { b }
    else                  { input[4] }
}

fn r6(value: f64) -> f64 {
    let a = value.abs();
    if a < 1e12 {
        if      a >= 1e-2 { (value*1e6).round()/1e6 }
        else if a >= 1e-4 { (value*1e8).round()/1e8 }
        else              { value }
    }
    else { value }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sampled {
    pub mean: f64,
    pub sem: f64,
    pub n: u64
}

impl Sampled {
    pub fn zero() -> Self { Sampled{ mean: std::f64::NAN, sem: std::f64::NAN, n: 0 } }
}

impl From<average::Variance> for Sampled {
    fn from(v: average::Variance) -> Sampled { Sampled { mean: r6(v.mean()), sem: r6(v.error()), n: v.len() } }
}

impl Display for Sampled {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} {} {}", self.n, self.mean, self.sem)
    }
}

impl Entitled for Sampled {
    fn push_subtitle(&self, specifier: &str, to: &mut String) {
        to.push_str(specifier); to.push_str("n ");
        to.push_str(specifier); to.push_str("mean ");
        to.push_str(specifier); to.push_str("sem");
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Speed {
    #[serde(flatten)]
    pub stats: Sampled,
    
    pub max: f64
}

impl Speed {
    pub fn zero() -> Speed { Speed{ stats: Sampled::zero(), max: std::f64::NAN } }
}

impl From<Speed> for Sampled {
    fn from(sp: Speed) -> Sampled { sp.stats }
}

impl From<&Speed> for Sampled {
    fn from(sp: &Speed) -> Sampled { sp.stats.clone() }
}

impl From<(average::Variance, f64)> for Speed {
    fn from(tup: (average::Variance, f64)) -> Speed {
        Speed{ stats: tup.0.into(), max: tup.1 }
    }
}

impl Display for Speed {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} {}", self.stats, self.max)
    }
}

impl Entitled for Speed {
    fn push_subtitle(&self, specifier: &str, to: &mut String) {
        self.stats.push_subtitle(specifier, to);
        to.push_str(" ");
        to.push_str(specifier); to.push_str("max");
    }
}

pub fn the_speed_in(t0: f64, t1: f64, input: &Vec<DataLine>) -> Option<Speed> {
    let mut stats = average::Variance::new();
    let mut five = [0f64; 5];
    let mut max_s = 0f64;
    let mut j = 0;
    let mut n = 0;
    let mut i = input.iter();
    let mut before = false;
    while let Some(data) = i.next() {
        if data.time < t0 { before = true; }
        else if data.time > t1 { 
            return { 
                if before && n >= 5 { Some((stats, max_s).into()) } 
                else { None } 
            }; 
        }
        else {
            if data.speed.is_finite() {
                stats.add(data.speed);
                five[j] = data.speed;
                n += 1;
                j += 1;
                if j >= 5 { j = 0; };
                if n >= 5 {
                    let s = median5(&five);
                    if s > max_s { max_s = s; };
                }
            }
        }
    }
    None
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Coord {
    pub first: f64,
    pub last: f64,
    pub bound0: f64,
    pub bound1: f64,

    #[serde(flatten)]
    pub stats: Sampled
}

impl Coord {
    pub fn zero() -> Coord { 
        Coord { first: std::f64::NAN, last: std::f64::NAN, bound0: std::f64::NAN, bound1: std::f64::NAN, stats: Sampled::zero() }
    }
}

impl Display for Coord {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} {} {} {} {}", self.first, self.last, self.bound0, self.bound1, self.stats)
    }
}

impl Entitled for Coord {
    fn push_subtitle(&self, specifier: &str, to: &mut String) {
        to.push_str(specifier); to.push_str("first ");
        to.push_str(specifier); to.push_str("last ");
        to.push_str(specifier); to.push_str("smallest ");
        to.push_str(specifier); to.push_str("largest ");
        self.stats.push_subtitle(specifier, to);
    }
}

pub fn the_coord<F>(f: F, input: &Vec<DataLine>) -> Coord
where F: Fn(&DataLine) -> f64 {
    if input.len() == 0 { return Coord::zero(); }

    let mut i = input.iter().map(f);
    let mut anything = false;
    let mut first = std::f64::NAN;
    let mut last = std::f64::NAN;
    let mut bound0 = std::f64::NAN;
    let mut bound1 = std::f64::NAN;
    let mut stats = average::Variance::new();
    while let Some(a) = i.next() {
        if a.is_finite() {
            if !anything {
                anything = true;
                first = a;
                bound0 = a;
                bound1 = a;
            }
            else {
                if a < bound0 { bound0 = a; }
                if a > bound1 { bound1 = a; }
            }
            last = a;
            stats.add(a);
        }
    }
    if anything { Coord{ first, last, bound0, bound1, stats: stats.into() } }
    else { Coord::zero() }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Scores {
    pub id: u32,
    pub t0: f64,
    pub t1: f64,
    pub area: Sampled,
    pub midline: Sampled,

    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub initial_speed: Option<Speed>,

    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub calm_speed: Option<Speed>,

    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub aroused_speed: Option<Speed>,

    pub x: Coord,
    pub y: Coord,
}

impl Scores {
    pub fn zero() -> Self {
        Scores{ 
            id: 0,
            t0: std::f64::NAN,
            t1: std::f64::NAN,
            area: Sampled::zero(),
            midline: Sampled::zero(),
            initial_speed: None,
            calm_speed: None,
            aroused_speed: None,
            x: Coord::zero(),
            y: Coord::zero(),
        }
    }
}

impl Display for Scores {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} {} {} {} {} {} {} {} {} {}",
            self.id, self.t0, self.t1,
            self.area, self.midline,
            self.initial_speed.clone().unwrap_or(Speed::zero()),
            self.calm_speed.clone().unwrap_or(Speed::zero()),
            self.aroused_speed.clone().unwrap_or(Speed::zero()),
            self.x, self.y
        )
    }
}

impl Entitled for Scores {
    fn push_subtitle(&self, specifier: &str, to: &mut String) {
        to.push_str(specifier); to.push_str("id ");
        to.push_str(specifier); to.push_str("t0 ");
        to.push_str(specifier); to.push_str("t1");
        let mock = Speed::zero();
        if specifier.len() == 0 {
            to.push_str(" "); self.area.push_subtitle("area-", to);
            to.push_str(" "); self.midline.push_subtitle("midline-", to);
            to.push_str(" "); mock.push_subtitle("initial-", to);
            to.push_str(" "); mock.push_subtitle("calm-", to);
            to.push_str(" "); mock.push_subtitle("aroused-", to);
            to.push_str(" "); self.x.push_subtitle("x-", to);
            to.push_str(" "); self.y.push_subtitle("y-", to);
        }
        else {
            let mut sub = String::new();
            sub.push_str(specifier);
            let n = sub.len();

            to.push_str(" "); sub.truncate(n); sub.push_str("area-");    self.area.push_subtitle(sub.as_str(), to);
            to.push_str(" "); sub.truncate(n); sub.push_str("midline-"); self.midline.push_subtitle(sub.as_str(), to);
            to.push_str(" "); sub.truncate(n); sub.push_str("initial-"); mock.push_subtitle(sub.as_str(), to);
            to.push_str(" "); sub.truncate(n); sub.push_str("calm-");    mock.push_subtitle(sub.as_str(), to);
            to.push_str(" "); sub.truncate(n); sub.push_str("aroused-"); mock.push_subtitle(sub.as_str(), to);
            to.push_str(" "); sub.truncate(n); sub.push_str("x-");       self.x.push_subtitle(sub.as_str(), to);
            to.push_str(" "); sub.truncate(n); sub.push_str("y-");       self.y.push_subtitle(sub.as_str(), to);
        }
    }
}

pub fn the_everything(id: u32, input: &Vec<DataLine>) -> Scores {
    if input.len() == 0 { return Scores::zero(); }

    let mut i0 = 0;
    let mut i1 = input.len() - 1;
    while i0 <  i1 && !input[i0].time.is_finite() { i0 += 1; }
    while i1 >= i0 && !input[i1].time.is_finite() { i1 -= 1; }
    if i1 < i0 { return Scores::zero(); }
    let t0 = input[i0].time;
    let t1 = input[i1].time;

    let area: Sampled = the_area(input).into();
    let midline: Sampled = the_midline(input).into();
    let initial_speed = the_speed_in(10.0, 20.0, input);
    let calm_speed = the_speed_in(270.0, 290.0, input);
    let aroused_speed = the_speed_in(440.0, 450.0, input);
    let x = the_coord(|d| d.x, input);
    let y = the_coord(|d| d.y, input);

    Scores{ id, t0, t1, area, midline, initial_speed, calm_speed, aroused_speed, x, y }
}
