use fastcgi::Request;
use mysql::prelude::*;
use mysql::*;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, io::Write, net::TcpListener};
use tera::{Context, Tera};

#[derive(Deserialize)]
struct Config {
    tcp: bool,
    db_string: String,
}

#[derive(Serialize, Deserialize)]
struct Trip {
    id: u32,
    waterway: String,
    county: String,
    start: String,
    finish: Option<String>,
    contributor: Option<String>,
}

impl FromRow for Trip {
    fn from_row_opt(row: Row) -> std::result::Result<Self, FromRowError>
    where
        Self: Sized,
    {
        Ok(Trip {
            id: row.get("id").ok_or(FromRowError(row.clone()))?,
            waterway: row.get("waterway").ok_or(FromRowError(row.clone()))?,
            county: row.get("county").ok_or(FromRowError(row.clone()))?,
            start: row.get("start").ok_or(FromRowError(row.clone()))?,
            finish: row.get("finish").ok_or(FromRowError(row.clone()))?,
            contributor: row.get::<Option<String>, _>("name").ok_or(FromRowError(row.clone()))?.map(|s| s.trim().to_owned()),
        })
    }
}

fn sort_page_handler(mut req: Request, tera: &Tera, mut conn: PooledConn) {
    let qs = req.param("QUERY_STRING").expect("No QUERY_STRING param");
    let params: HashMap<&str, &str> = querystring::querify(&qs).into_iter().collect();
    let mut trips: Vec<Trip> = conn
        .query("SELECT id,name,county,waterway,start,finish,date from trips where active = 1")
        .expect("MySQL query failed");

    let mut context = Context::new();

    if params.get("sort") == Some(&"county") {
        trips.sort_by_key(|trip| trip.county.clone());
        context.insert("sort_type", "county");
    } else {
        trips.sort_by_key(|trip| trip.waterway.clone());
        context.insert("sort_type", "waterway");
    }
    context.insert("trips", &trips);
    write!(
        &mut req.stdout(),
        "Content-Type: text/html\n\n{}",
        tera.render("sorttemplate.htm", &context)
            .expect("rendering failure")
    )
    .unwrap_or(());
}

fn main() {
    let mut config_path = dirs::config_dir().expect("No config dir found");
    config_path.push("cdo.toml");
    let config_str = std::fs::read_to_string(config_path).expect("No config file found");
    let config: Config = toml::from_str(&config_str).expect("Could not parse config file");
    let sort_page = include_str!("../templates/sorttemplate.htm");

    let mut tera = Tera::default();
    tera.add_raw_template("sorttemplate.htm", &sort_page)
        .expect("Could not parse template");

    let pool = Pool::new(config.db_string).expect("MySQL pool setup failed");

    if config.tcp {
        let listener = TcpListener::bind("127.0.0.1:9000").expect("Could not bind to TCP port");
        fastcgi::run_tcp(
            move |req| {
                sort_page_handler(
                    req,
                    &tera,
                    pool.get_conn().expect("MySQL connection failed"),
                )
            },
            &listener,
        );
    } else {
        fastcgi::run(move |req| {
            sort_page_handler(
                req,
                &tera,
                pool.get_conn().expect("MySQL connection failed"),
            )
        });
    }
}
