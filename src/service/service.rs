use rocket::{Build, Rocket};

#[macro_use] extern crate rocket;


#[rocket::main]
async fn main() {
    let _ = rocket().launch().await;
}

fn rocket() -> Rocket<Build> {
   rocket::build()
        .mount("/", routes![index, hello])
}

#[get("/")]
fn index() -> &'static str {
    "Hello World!"
}

#[get("/hello/<name>")]
fn hello(name: &str) -> String {
    format!("Hello, {}!", name)
}
