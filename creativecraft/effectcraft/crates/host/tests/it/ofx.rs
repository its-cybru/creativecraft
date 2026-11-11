//! OpenFX plug-ins end to end, on a machine that has one installed.
//!
//! These are the paths the handoff listed as "never tested": a plug-in parameter
//! is an ordinary animatable property, it survives save/reopen, and a Render
//! Queue export renders through it. Each test skips with a message on a machine
//! without the bundle, so the suite stays green elsewhere.

use effectcraft_engine::Session;
use effectcraft_engine::time::Tick;
use serde_json::json;

const TONELAB: &str = r"C:\Program Files\Common Files\OFX\Plugins\ToneLab.ofx.bundle\Contents\Win64\ToneLab.ofx";
const TRIOSCOPE: &str = r"C:\Program Files\TrioscopeSEE\2.1\bin\see.2.1_relight.1.0_openfx.ofx";

fn px(s: &mut Session, seconds: f64) -> [f32; 4] {
    let cid = s.active_comp_id().expect("comp");
    s.render(cid, Tick::from_seconds_f64(seconds), Default::default()).get(32, 32)
}

fn setup() -> (Session, u64) {
    let mut s = effectcraft_host::session();
    s.execute("comp.new", json!({"name": "OFX", "width": 64, "height": 64, "duration": 2, "frameRate": 24})).unwrap();
    let l = s.execute("layer.newSolid", json!({"color": [0.2, 0.4, 0.6], "width": 64, "height": 64})).unwrap()["layer"].as_u64().unwrap();
    (s, l)
}

#[test]
fn ofx_parameters_animate_and_survive_save_reopen() {
    if !std::path::Path::new(TONELAB).is_file() {
        eprintln!("skipping: ToneLab is not installed");
        return;
    }
    assert_eq!(effectcraft_ofx::load_and_register(std::path::Path::new(TONELAB)).unwrap(), 1, "ToneLab registers");

    let (mut s, l) = setup();
    s.execute("effect.apply", json!({"layer": l, "effect": "com.tonelab"})).unwrap();
    let at = "effects/#1/Exposure";
    // The effect must be on for Exposure to show.
    s.execute("prop.set", json!({"layer": l, "path": "effects/#1/BasicEnable", "value": true})).unwrap();
    let plain = px(&mut s, 0.0);
    s.execute("prop.set", json!({"layer": l, "path": at, "value": 2.0})).unwrap();
    let exposed = px(&mut s, 0.0);
    assert_ne!(plain, exposed, "Exposure 2.0 must change the frame (plain {plain:?}, exposed {exposed:?})");
    let saved_value = s.execute("prop.get", json!({"layer": l, "path": at})).unwrap()["value"].clone();
    assert!(saved_value.as_f64().unwrap_or_default() > 1.9, "{saved_value}");

    // 1. Save and reopen: the plug-in id and the parameter value come back.
    let dir = std::env::temp_dir().join(format!("effectcraft-ofx-it-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("ofx.ecproj");
    s.execute("file.save", json!({"path": file.to_string_lossy()})).unwrap();
    let mut reopened = effectcraft_host::session();
    reopened.execute("file.open", json!({"path": file.to_string_lossy()})).unwrap();
    let reopened_value = reopened.execute("prop.get", json!({"layer": l, "path": at})).unwrap()["value"].clone();
    assert_eq!(reopened_value, saved_value, "the Exposure value survives save/reopen");
    assert_ne!(px(&mut reopened, 0.0), plain, "the reopened project still renders the OFX effect");

    // 2. Keyframes: animate Exposure from 0.5 s to 1.5 s and read it at three times.
    reopened.execute("prop.toggleAnimation", json!({"layer": l, "path": at})).unwrap();
    reopened.execute("prop.set", json!({"layer": l, "path": at, "value": 0.0, "time": 0.5})).unwrap();
    reopened.execute("prop.set", json!({"layer": l, "path": at, "value": 3.0, "time": 1.5})).unwrap();
    let start = reopened.execute("prop.get", json!({"layer": l, "path": at, "time": 0.5})).unwrap()["value"].as_f64().unwrap_or_default();
    let middle = reopened.execute("prop.get", json!({"layer": l, "path": at, "time": 1.0})).unwrap()["value"].as_f64().unwrap_or_default();
    let end = reopened.execute("prop.get", json!({"layer": l, "path": at, "time": 1.5})).unwrap()["value"].as_f64().unwrap_or_default();
    assert!((start - 0.0).abs() < 0.01 && (middle - 1.5).abs() < 0.05 && (end - 3.0).abs() < 0.01, "animated plug-in parameter: {start} {middle} {end}");
    let keyed_a = px(&mut reopened, 0.5);
    let keyed_b = px(&mut reopened, 1.5);
    assert_ne!(keyed_a, keyed_b, "the animated plug-in parameter changes the rendered frames");

    // 3. Render Queue: one frame through the exporter, PNG on disk.
    let out = dir.join("ofx_render.png");
    reopened
        .execute(
            "renderQueue.add",
            json!({"comp": "OFX", "format": "png", "output": out.to_string_lossy(), "timeSpan": "custom", "start": 1.5, "end": 1.5 + 1.0 / 24.0}),
        )
        .unwrap();
    reopened.execute("renderQueue.render", json!({"wait": true})).unwrap();
    let queue = reopened.execute("renderQueue.list", json!({})).unwrap();
    println!("render queue after render: {queue}");
    // A PNG Sequence names each frame after its frame number (`ofx_render_00036.png`),
    // so trust the queue's own `last_output` rather than the requested name.
    let written = queue["items"][0]["last_output"].as_str().map(std::path::PathBuf::from).filter(|p| p.is_file()).unwrap_or(out);
    assert!(written.is_file(), "the Render Queue wrote {} (queue {queue})", written.display());
    let bytes = std::fs::read(&written).unwrap();
    let image = image::load_from_memory(&bytes).expect("a PNG").to_rgb8();
    let frame = px(&mut reopened, 1.5);
    let sample = image.get_pixel(32, 32);
    let to_u8 = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    let close = |a: u8, b: u8| a.abs_diff(b) <= 3;
    assert!(
        close(to_u8(frame[0]), sample[0]) && close(to_u8(frame[1]), sample[1]) && close(to_u8(frame[2]), sample[2]),
        "the exported frame matches the viewer: {frame:?} vs {:?}",
        sample.0
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// Trioscope at full HD: the probe was 32x32, so this is the "does it render a
/// real frame" check. Heavy (raytracing), so opt in with `--ignored`.
#[test]
#[ignore = "heavy: full-size raytracing plug-in render"]
fn trioscope_renders_a_full_hd_frame() {
    if !std::path::Path::new(TRIOSCOPE).is_file() {
        eprintln!("skipping: Trioscope is not installed");
        return;
    }
    // Run it where the application runs it: in the helper process. Trioscope's
    // DLL cannot be unloaded cleanly in-process (the process hangs or faults on
    // exit), which is exactly why the host isolates plug-ins in a helper.
    let shell = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../studio-shell/target/debug/studio-shell.exe");
    if !shell.is_file() {
        eprintln!("skipping: build studio-shell first ({})", shell.display());
        return;
    }
    effectcraft_ofx::configure_worker(&shell);
    let registered = effectcraft_ofx::describe_and_register(std::path::Path::new(TRIOSCOPE));
    assert!(registered >= 1, "Trioscope describes through the helper");
    let mut s = effectcraft_host::session();
    s.execute("comp.new", json!({"name": "T", "width": 1920, "height": 1080, "duration": 1, "frameRate": 24})).unwrap();
    let l = s.execute("layer.newSolid", json!({"color": [0.3, 0.5, 0.7], "width": 1920, "height": 1080})).unwrap()["layer"].as_u64().unwrap();
    s.execute("effect.apply", json!({"layer": l, "effect": "trioscope_see_2_1_Raytracing_Relight_1_0"})).unwrap();
    let start = std::time::Instant::now();
    let f = px(&mut s, 0.0);
    let elapsed = start.elapsed();
    println!("Trioscope 1920x1080 through the helper: {elapsed:?} for one frame, pixel {f:?}");
    assert!(f[3] > 0.99, "a full frame came back");
    effectcraft_ofx::shutdown();
}
