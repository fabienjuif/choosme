// THIS FILE IS NOT USED YET
// THE INTENT IS TO HAVE A SOLUTION TO PLACE THE WINDOW NEAR THE CURSOR

fn get_cursor_position() -> Option<(i32, i32)> {
    let out = Command::new("slurp")
        .arg("-p")
        .arg("-f")
        .arg("%x %y") // only get cursor coordinates, no selection
        .output();

    if let Err(e) = out {
        eprintln!("Failed to execute swaymsg: {}", e);
        return None;
    }

    println!("out: {:?}", out);

    let json: serde_json::Value = serde_json::from_slice(&out.ok()?.stdout).ok()?;
    let cursor = &json[0]["cursor"];
    Some((cursor["x"].as_f64()? as i32, cursor["y"].as_f64()? as i32))
}
