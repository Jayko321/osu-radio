#[tokio::main]
async fn main() {
    let res = radio_scanner::helpers::find_osu_markers();
    println!("{res:?}");
    let Some(marker) = res.first() else {
        eprintln!("no osu! installations found");
        return;
    };

    match radio_scanner::get_beatmaps(marker.clone()).await {
        Ok(beatmaps) => println!("{:?}", beatmaps.get(2)),
        Err(error) => eprintln!("failed to import beatmaps: {error}"),
    }
}
