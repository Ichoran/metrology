// This file is distributed under the BSD 3-clause license.  See file LICENSE.
// Copyright (c) 2022 Rex Kerr and Calico Life Sciences LLC


use nom::*;


#[derive(Debug, Clone)]
pub struct DataLine {
    pub time: f64,
    pub area: f64,
    pub speed: f64,
    pub midline: f64,
    pub x: f64,
    pub y: f64
}

named!(double_at_end<f64>,
    map_res!(map_res!(rest, std::str::from_utf8), |s: &str| s.parse::<f64>())
);

named!(java_double<f64>,
    alt_complete!(
        double
        | map!(tag_s!("NaN"), |_| std::f64::NAN)
        | map!(tag_s!("-Infinity"), |_| std::f64::NEG_INFINITY)
        | map!(tag_s!("Infinity"), |_| std::f64::INFINITY)
        | double_at_end
    )
);

fn token_end(input: &[u8]) -> IResult<&[u8], ()> {
    if input.len() == 0 { Ok((input, ())) }
    else { 
        match input[0] as char {
            ' ' | '\t' | '\n' | '\r' => {
                let n = input.position(|c| c == '\n' as u8)
                    .map(|k| k+1)
                    .unwrap_or(input.len());
                Ok((&input[n..], ()))
            }
            _  =>
                Err(nom::Err::Error(error_position!(input, ErrorKind::Eof)))
        }
    }
}

named!(get_data_line<DataLine>,
    do_parse!(
        time: java_double >>
        multispace >>
        area: java_double >>
        multispace >>
        speed: java_double >>
        multispace >>
        midline: java_double >>
        multispace >>
        x: java_double >>
        multispace >>
        y: java_double >>
        token_end >>
        (DataLine{ time, area, speed, midline, x, y })
    )
);

named!(pub get_data_lines< Vec<DataLine> >,
    many1!(get_data_line)
);
