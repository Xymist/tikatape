use crate::{Format, Input};

use color_eyre::{eyre::eyre, Result};
use std::{
    collections::HashMap,
    fs::File,
    io::Write,
    path::PathBuf,
    process::{Command, Stdio},
};
use tempdir::TempDir;

const TIKA_NAME: &str = "tika-app-2.4.0.jar";
const TIKA_JAR: &[u8] = include_bytes!("../jar/tika-app-2.4.0.jar");
const TIKA_OCR_CONFIG: &[u8] = include_bytes!("../jar/tika-config.xml");
const TIKA_NO_OCR_CONFIG: &[u8] = include_bytes!("../jar/tika-config-without-ocr.xml");

/// Use bundled Tika. This uses a vendored JAR and boots it using the
/// local system's Java for each new data request. Simple relative
/// to running a server but slower and less easy to update in isolation.
pub struct Client {
    input: Input,
    ocr: bool,
    jar_path: Option<TempDir>,
}

impl Client {
    pub fn try_new(input: Input, ocr: bool) -> Result<Self> {
        Ok(Client {
            input,
            ocr,
            jar_path: Some(jar_path()?),
        })
    }

    fn data_source(&mut self) -> Result<String> {
        Ok(match self.input {
            Input::FilePath(ref file) => file
                .as_os_str()
                .to_str()
                .ok_or(eyre!("Invalid file path"))?
                .to_owned(),
            Input::Url(ref url) => url.as_str().to_owned(),
        })
    }

    fn tmp_jar_cmd(&self) -> Option<String> {
        Some(
            (self
                .jar_path
                .as_ref()?
                .path()
                .join(TIKA_NAME)
                .as_os_str()
                .to_str()?)
            .to_string(),
        )
    }

    fn config(&self) -> Result<PathBuf> {
        let config = if self.ocr {
            TIKA_OCR_CONFIG
        } else {
            TIKA_NO_OCR_CONFIG
        };
        let path = self.jar_path.as_ref().ok_or(eyre!("Missing tmp folder"))?;
        let file_path = path.path().join("tika-config.xml");
        let mut tmp_file = File::create(file_path.clone())?;
        tmp_file.write_all(config).unwrap();
        Ok(file_path)
    }

    pub fn text(&mut self) -> Result<String> {
        let res = self.process(Format::Text)?;
        text_result(res)
    }

    pub fn html(&mut self) -> Result<String> {
        let res = self.process(Format::Html)?;
        text_result(res)
    }

    pub fn mimetype(&mut self) -> Result<mime::Mime> {
        let res = self.process(Format::Mime)?;
        res.get("Content-Type")
            .and_then(|content| content.as_str())
            .and_then(|s| s.parse().ok())
            .ok_or(eyre!("Missing or empty content"))
    }

    pub fn metadata(&mut self) -> Result<HashMap<String, serde_json::Value>> {
        self.process(Format::Metadata)
    }

    fn process(&mut self, format: Format) -> Result<HashMap<String, serde_json::Value>> {
        let child = Command::new(java_bin().as_str())
            .arg("-Djava.awt.headless=true")
            .arg("-jar")
            .arg(self.tmp_jar_cmd().ok_or(eyre!("Missing tmp folder"))?)
            .arg(&format!(
                "--config={}",
                self.config()?
                    .as_os_str()
                    .to_str()
                    .ok_or(eyre!("No config for Tika available"))?
            ))
            .arg(format_arg(format))
            .arg(self.data_source()?)
            .stdout(Stdio::piped())
            .spawn()
            .expect("Error spawning Tika server process");

        let result = child.wait_with_output()?;

        process_output(std::str::from_utf8(&result.stdout)?.to_owned(), format)
    }
}

fn text_result(res: HashMap<String, serde_json::Value>) -> Result<String> {
    res.get("result")
        .and_then(|content| content.as_str())
        .map(ToOwned::to_owned)
        .ok_or(eyre!("Missing or empty content"))
}

fn process_output(output: String, format: Format) -> Result<HashMap<String, serde_json::Value>> {
    match format {
        Format::Html | Format::Text => {
            let mut res = HashMap::new();
            res.insert("result".into(), serde_json::Value::String(output));
            Ok(res)
        }
        Format::Mime => {
            let mut s: HashMap<String, serde_json::Value> = serde_json::from_str(output.as_str())?;
            s.retain(|k, _| k == "Content-Type");
            Ok(s)
        }
        Format::Metadata => {
            let res: HashMap<String, serde_json::Value> = serde_json::from_str(output.as_str())?;
            Ok(res)
        }
    }
}

fn format_arg(format: Format) -> &'static str {
    match format {
        Format::Html => "-h",
        Format::Text => "-t",
        Format::Mime | Format::Metadata => "-j",
    }
}

fn jar_path() -> Result<TempDir> {
    let tmp_dir = TempDir::new("tika_jar")?;
    let file_path = tmp_dir.path().join(TIKA_NAME);
    let mut tmp_file = File::create(file_path)?;
    tmp_file.write_all(TIKA_JAR).unwrap();
    Ok(tmp_dir)
}

fn java_bin() -> String {
    let java_home = std::env::var("JAVA_HOME");

    if let Ok(java_home) = java_home {
        format!("{}/bin/java", java_home)
    } else {
        "java".to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_process_url() {
        let mut client =
            Client::try_new(Input::Url("https://example.com".parse().unwrap()), true).unwrap();
        println!("{:?}", client.html().unwrap());
        println!("{:?}", client.metadata().unwrap());
        println!("{:?}", client.mimetype().unwrap());
    }

    #[test]
    fn test_process_file() {
        let mut client =
            Client::try_new(Input::FilePath("Cargo.toml".parse().unwrap()), true).unwrap();
        println!("{:?}", client.text().unwrap());
        println!("{:?}", client.metadata().unwrap());
        println!("{:?}", client.mimetype().unwrap());
    }
}
