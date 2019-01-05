extern crate rumble;
extern crate ruuvi_sensor_protocol;
extern crate structopt;

use std::collections::BTreeMap;
use std::io::Write;
use std::time::SystemTime;
use structopt::StructOpt;

pub mod ruuvi;
use ruuvi::{on_measurement, Measurement};

pub mod influxdb;
use influxdb::{DataPoint, FieldValue};

use std::alloc::System;

#[global_allocator]
static GLOBAL: System = System;

fn tag_set(
    aliases: &BTreeMap<String, String>,
    measurement: &Measurement,
) -> BTreeMap<String, String> {
    let mut tags = BTreeMap::new();
    let address = measurement.address.to_string();
    tags.insert(
        "name".to_string(),
        aliases.get(&address).unwrap_or(&address).to_string(),
    );
    tags
}

macro_rules! to_float {
    ( $value: expr, $scale: expr ) => {{
        FieldValue::FloatValue(f64::from($value) / $scale)
    }};
}

macro_rules! add_value {
    ( $fields: ident, $value: expr, $field: expr, $scale: expr ) => {{
        if let Some(value) = $value {
            $fields.insert($field.to_string(), to_float!(value, $scale));
        }
    }};
}

fn field_set(measurement: &Measurement) -> BTreeMap<String, FieldValue> {
    let mut fields = BTreeMap::new();
    add_value!(
        fields,
        measurement.sensor_values.temperature,
        "temperature",
        1000.0
    );
    add_value!(
        fields,
        measurement.sensor_values.humidity,
        "humidity",
        10000.0
    );
    add_value!(
        fields,
        measurement.sensor_values.pressure,
        "pressure",
        1000.0
    );
    add_value!(
        fields,
        measurement.sensor_values.battery_potential,
        "battery_potential",
        1000.0
    );

    if let Some(ref acceleration) = measurement.sensor_values.acceleration {
        fields.insert(
            "acceleration_x".to_string(),
            to_float!(acceleration.0, 1000.0),
        );
        fields.insert(
            "acceleration_y".to_string(),
            to_float!(acceleration.1, 1000.0),
        );
        fields.insert(
            "acceleration_z".to_string(),
            to_float!(acceleration.2, 1000.0),
        );
    }

    fields
}

fn to_data_point(
    aliases: &BTreeMap<String, String>,
    name: String,
    measurement: &Measurement,
) -> DataPoint {
    DataPoint {
        measurement: name,
        tag_set: tag_set(aliases, &measurement),
        field_set: field_set(&measurement),
        timestamp: Some(SystemTime::now()),
    }
}

#[derive(Debug)]
pub struct Alias {
    pub address: String,
    pub name: String,
}

fn parse_alias(src: &str) -> Result<Alias, &str> {
    let index = src.find('=');
    match index {
        Some(i) => {
            let (address, name) = src.split_at(i);
            Ok(Alias {
                address: address.to_string(),
                name: name.get(1..).unwrap_or("").to_string(),
            })
        }
        None => Err("invalid alias"),
    }
}

fn alias_map(aliases: &[Alias]) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    for alias in aliases.iter() {
        map.insert(alias.address.to_string(), alias.name.to_string());
    }
    map
}

#[derive(Debug, StructOpt)]
#[structopt(rename_all = "kebab-case")]
struct Options {
    #[structopt(long, default_value = "ruuvi_measurement")]
    /// The name of the measurement in InfluxDB line protocol.
    influxdb_measurement: String,
    #[structopt(long, parse(try_from_str = "parse_alias"))]
    /// Specify human-readable alias for RuuviTag id. For example --alias DE:AD:BE:EF:00:00=Sauna.
    alias: Vec<Alias>,
}

fn listen(options: Options) {
    let name = options.influxdb_measurement;
    let aliases = alias_map(&options.alias);
    on_measurement(Box::new(move |measurement| {
        match writeln!(
            std::io::stdout(),
            "{}",
            to_data_point(&aliases, name.to_string(), &measurement)
        ) {
            Ok(_) => (),
            Err(error) => {
                eprintln!("error: {}", error);
                ::std::process::exit(1);
            }
        }
    }));
}

fn main() {
    let options = Options::from_args();
    listen(options)
}
