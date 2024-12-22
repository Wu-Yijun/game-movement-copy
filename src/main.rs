mod player;
mod recorder;
mod state;
use recorder::Recorder;

// fn run_vigem() {
//     use std::{thread, time};
//     // Connect to the ViGEmBus driver
//     let client = vigem_client::Client::connect().unwrap();

//     // Create the virtual controller target
//     let id = vigem_client::TargetId::XBOX360_WIRED;
//     let mut target = vigem_client::Xbox360Wired::new(client, id);

//     // Plugin the virtual controller
//     target.plugin().unwrap();

//     // Wait for the virtual controller to be ready to accept updates
//     target.wait_ready().unwrap();

//     // The input state of the virtual controller
//     let mut gamepad = vigem_client::XGamepad {
//         buttons: vigem_client::XButtons!(UP | RIGHT | LB | A | X),
//         ..Default::default()
//     };

//     let start = time::Instant::now();
//     loop {
//         let elapsed = start.elapsed().as_secs_f64();

//         // Play for 10 seconds
//         if elapsed >= 10.0 {
//             break;
//         }

//         // Spin the left thumb stick in circles
//         gamepad.thumb_lx = (elapsed.cos() * 30000.0) as i16;
//         gamepad.thumb_ly = (elapsed.sin() * 30000.0) as i16;

//         // Spin the right thumb stick in circles
//         gamepad.thumb_rx = -gamepad.thumb_ly;
//         gamepad.thumb_ry = gamepad.thumb_lx;

//         // Twiddle the triggers
//         gamepad.left_trigger = ((((elapsed * 1.5).sin() * 127.0) as i32) + 127) as u8;
//         gamepad.right_trigger = ((((elapsed * 1.5).cos() * 127.0) as i32) + 127) as u8;

//         let _ = target.update(&gamepad);

//         thread::sleep(time::Duration::from_millis(10));
//     }
// }

#[cfg(windows)]
fn main() {
    env_logger::builder()
        .target(env_logger::Target::Stdout)
        .filter_level(log::LevelFilter::Warn)
        // .filter_level(log::LevelFilter::Info)
        // .filter_level(log::LevelFilter::Debug)
        .init();
    // log::warn!("This info message will always be shown");
    // return;
    let mut record = Recorder::from_file("config.yaml".to_string());
    // println!("{:#?}", record);
    record.save_to_file("config.yaml".to_string());
    record.init();
    while record.is_ok() {
        record.listen();
        record.match_shortcuts();
    }
}

#[test]
fn test_screen() {
    let (w, h) = rdev::display_size().unwrap();
    println!(
        "My screen size : {:?}x{:?}",
        w as f64 * 1.25,
        h as f64 * 1.25
    );
}
