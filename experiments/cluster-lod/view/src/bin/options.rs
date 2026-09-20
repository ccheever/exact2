use clod_view::{
    Mode, View,
    scene::{Camera, Scene},
};
use glam::Vec3;
use std::path::PathBuf;
pub struct Options {
    pub command: String,
    pub file: PathBuf,
    pub out: PathBuf,
    pub width: u32,
    pub height: u32,
    pub layout: String,
    pub mode: Mode,
    pub view: View,
    pub thresholds: Vec<f32>,
    pub times: Vec<f32>,
    pub steps: u32,
    pub frames: u32,
    pub eye: Option<Vec3>,
    pub target: Option<Vec3>,
    pub fov: f32,
}
impl Options {
    pub fn parse() -> Result<Self, String> {
        let mut args = std::env::args().skip(1);
        let command = args
            .next()
            .ok_or("usage: clod-view <render|time|compare|pop> <file.clod> [options]")?;
        if !["render", "time", "compare", "pop"].contains(&command.as_str()) {
            return Err("command must be render|time|compare|pop".into());
        }
        let file = args.next().ok_or("missing .clod file")?.into();
        let out = PathBuf::from(std::env::var("HOME").map_err(|e| e.to_string())?)
            .join("Library/Caches/exact2-cluster-lod/out");
        let mut result = Self {
            thresholds: if command == "compare" {
                vec![0.5, 1.0, 2.0, 4.0, 8.0]
            } else {
                vec![1.0]
            },
            times: if command == "compare" {
                vec![0.0, 0.25, 0.5, 0.75, 1.0]
            } else {
                vec![0.0]
            },
            command,
            file,
            out,
            width: 2560,
            height: 1440,
            layout: "single".into(),
            mode: Mode::Cluster,
            view: View::Lit,
            steps: 240,
            frames: 7,
            eye: None,
            target: None,
            fov: 45f32.to_radians(),
        };
        while let Some(arg) = args.next() {
            let value = args
                .next()
                .ok_or_else(|| format!("missing value for {arg}"))?;
            match arg.as_str() {
                "--out" => result.out = value.into(),
                "--layout" => result.layout = value,
                "--mode" => {
                    result.mode = match value.as_str() {
                        "cluster" => Mode::Cluster,
                        "naive" => Mode::Naive,
                        _ => return Err("mode must be cluster|naive".into()),
                    }
                }
                "--view" => result.view = View::parse(&value)?,
                "--threshold-px" => result.thresholds = numbers(&value)?,
                "--t" => result.times = numbers(&value)?,
                "--steps" => result.steps = value.parse().map_err(|_| "bad step count")?,
                "--frames" => result.frames = value.parse().map_err(|_| "bad frame count")?,
                "--size" => {
                    let (w, h) = value.split_once('x').ok_or("size must be WIDTHxHEIGHT")?;
                    result.width = w.parse().map_err(|_| "bad width")?;
                    result.height = h.parse().map_err(|_| "bad height")?;
                }
                "--path" => {
                    if value != "hero" {
                        return Err("only --path hero is supported".into());
                    }
                }
                "--eye" => result.eye = Some(vector(&value)?),
                "--target" => result.target = Some(vector(&value)?),
                "--fov" => result.fov = value.parse::<f32>().map_err(|_| "bad fov")?.to_radians(),
                _ => return Err(format!("unknown option {arg}")),
            }
        }
        let mut errors = Vec::new();
        if result
            .thresholds
            .iter()
            .any(|x| !x.is_finite() || *x < 0.0 || *x == f32::MAX)
        {
            errors.push("threshold must be finite, nonnegative, below f32::MAX");
        }
        if result
            .times
            .iter()
            .any(|x| !x.is_finite() || !(0.0..=1.0).contains(x))
        {
            errors.push("t must be in 0..1");
        }
        if result.steps < 2 || result.steps > 10000 {
            errors.push("steps must be 2..10000");
        }
        if result.frames < 1 || result.frames > 1000 {
            errors.push("frames must be 1..1000");
        }
        if !(0.05..3.0).contains(&result.fov) {
            errors.push("fov must be between 3 and 171 degrees");
        }
        if result.eye.is_some() != result.target.is_some() {
            errors.push("eye and target must be specified together");
        }
        if result
            .eye
            .zip(result.target)
            .is_some_and(|(e, t)| e.distance(t) < 1e-5 || (e - t).cross(Vec3::Z).length() < 1e-5)
        {
            errors.push("eye/target must define a nonvertical view");
        }
        if !errors.is_empty() {
            return Err(errors.join("; "));
        }
        Ok(result)
    }
    pub fn camera(&self, scene: &Scene, t: f32) -> Camera {
        if let (Some(eye), Some(target)) = (self.eye, self.target) {
            Camera::perspective(
                eye,
                target,
                self.width as f32 / self.height as f32,
                self.fov,
                0.002,
                scene.radius * 12.0 + eye.distance(scene.center) + 10.0,
            )
        } else {
            scene.camera(t, self.width as f32 / self.height as f32, self.fov)
        }
    }
    pub fn output_dir(&self) -> PathBuf {
        if self.out.extension().is_some() {
            self.out
                .parent()
                .unwrap_or_else(|| std::path::Path::new("."))
                .into()
        } else {
            self.out.clone()
        }
    }
}
fn numbers(s: &str) -> Result<Vec<f32>, String> {
    s.split(',')
        .map(|x| x.parse().map_err(|_| format!("invalid number {x}")))
        .collect()
}
fn vector(s: &str) -> Result<Vec3, String> {
    let v = numbers(s)?;
    if v.len() != 3 || v.iter().any(|x| !x.is_finite()) {
        return Err("vector needs three finite numbers".into());
    }
    Ok(Vec3::new(v[0], v[1], v[2]))
}
