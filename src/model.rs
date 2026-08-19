use std::path::{Path, PathBuf};

use serde::{Deserialize, Deserializer, de::Error};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetMode {
    Local,
    Remote,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildTarget {
    os: String,
    arch: String,
    destination: String,
    mode: TargetMode,
}

impl BuildTarget {
    pub fn local() -> Self {
        Self {
            os: Self::local_os(),
            arch: Self::local_arch(),
            destination: "local".to_string(),
            mode: TargetMode::Local,
        }
    }

    pub fn remote(os: &str, arch: &str, destination: &str) -> Self {
        Self {
            os: os.to_string(),
            arch: arch.to_string(),
            destination: destination.to_string(),
            mode: TargetMode::Remote,
        }
    }

    pub fn os(&self) -> &str {
        &self.os
    }

    pub fn arch(&self) -> &str {
        &self.arch
    }

    pub fn destination(&self) -> &str {
        &self.destination
    }

    #[cfg(test)]
    pub fn mode(&self) -> &TargetMode {
        &self.mode
    }

    pub fn is_local(&self) -> bool {
        matches!(self.mode, TargetMode::Local)
    }

    pub fn host(&self) -> &str {
        self.destination
            .split_once(':')
            .map(|(host, _)| host)
            .unwrap_or(self.destination())
    }

    pub fn remote_path(&self) -> &str {
        self.destination
            .split_once(':')
            .map(|(_, path)| path)
            .unwrap_or(self.destination())
    }

    pub fn make_cmd(&self) -> &str {
        match self.os() {
            "FreeBSD" => "gmake",
            _ => "make",
        }
    }

    pub fn log_key(&self) -> String {
        self.destination()
            .chars()
            .map(|ch| {
                if ch == '/' || ch == ':' || ch == '@' {
                    '_'
                } else {
                    ch
                }
            })
            .collect()
    }

    pub fn title(&self) -> String {
        if self.is_local() {
            format!("{} {} localhost", self.os(), self.arch())
        } else {
            format!("{} {} {}", self.os(), self.arch(), self.destination())
        }
    }

    fn local_os() -> String {
        match std::env::consts::OS {
            "linux" => "GNU/Linux".to_string(),
            "freebsd" => "FreeBSD".to_string(),
            "netbsd" => "NetBSD".to_string(),
            "openbsd" => "OpenBSD".to_string(),
            other => other.to_string(),
        }
    }

    fn local_arch() -> String {
        match std::env::consts::ARCH {
            "x86_64" => "x86_64".to_string(),
            "aarch64" => "aarch64".to_string(),
            other => other.to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResultMirrorPlan {
    enabled: bool,
    root: PathBuf,
    manifest: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildOutputPlan {
    sources: Vec<PathBuf>,
    files: Option<Vec<PathBuf>>,
    destination: PathBuf,
}

impl BuildOutputPlan {
    fn new(
        sources: Vec<String>,
        files: Option<Vec<String>>,
        destination: String,
    ) -> Result<Self, String> {
        let sources = Self::relative_paths(sources, "source")?;
        let files = files
            .map(|files| Self::relative_paths(files, "file"))
            .transpose()?;
        let destination = PathBuf::from(destination);

        if sources.is_empty() {
            return Err("invalid project build: src must contain at least one path".to_string());
        }
        if destination.as_os_str().is_empty() {
            return Err("invalid project build: dst must not be empty".to_string());
        }
        if !destination.is_absolute()
            && sources.iter().any(|source| destination.starts_with(source))
        {
            return Err(
                "invalid project build: dst must not be inside a configured src path".to_string(),
            );
        }

        Ok(Self {
            sources,
            files,
            destination,
        })
    }

    pub fn sources(&self) -> &[PathBuf] {
        &self.sources
    }

    pub fn destination(&self) -> &Path {
        &self.destination
    }

    pub fn files(&self) -> &[PathBuf] {
        self.files.as_deref().unwrap_or_default()
    }

    pub fn selects_files(&self) -> bool {
        self.files.is_some()
    }

    fn relative_paths(paths: Vec<String>, kind: &str) -> Result<Vec<PathBuf>, String> {
        paths
            .into_iter()
            .map(PathBuf::from)
            .map(|path| {
                if Self::is_workspace_path(&path) {
                    Ok(path)
                } else {
                    Err(format!(
                        "invalid project build {kind} '{}': expected a non-empty relative path without '..'",
                        path.display()
                    ))
                }
            })
            .collect()
    }

    fn is_workspace_path(path: &Path) -> bool {
        !path.as_os_str().is_empty()
            && !path.is_absolute()
            && path.file_name().is_some()
            && !path.components().any(|component| {
                matches!(
                    component,
                    std::path::Component::ParentDir | std::path::Component::RootDir
                )
            })
    }
}

impl ResultMirrorPlan {
    pub fn new(enabled: bool, root: PathBuf, entry: &str) -> Self {
        Self {
            enabled,
            root,
            manifest: Self::manifest_for_entry(entry),
        }
    }

    #[cfg(test)]
    pub fn disabled(root: PathBuf, entry: &str) -> Self {
        Self::new(false, root, entry)
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn manifest(&self) -> &Path {
        &self.manifest
    }

    fn manifest_for_entry(entry: &str) -> PathBuf {
        PathBuf::from("build")
            .join(".mxrun")
            .join(format!("{entry}.paths"))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MxrunConfig {
    targets: Vec<BuildTarget>,
    ignores: Vec<String>,
    build_output: Option<BuildOutputPlan>,
}

impl MxrunConfig {
    pub fn parse(src: &str) -> Result<Self, String> {
        if Self::is_yaml(src) {
            Self::from_yaml(src)
        } else {
            Self::from_legacy_lines(
                src.lines()
                    .enumerate()
                    .filter_map(Line::meaningful)
                    .map(Line::parse)
                    .collect::<Result<Vec<_>, _>>()?,
            )
        }
    }

    pub fn targets(&self) -> &[BuildTarget] {
        &self.targets
    }

    pub fn ignores(&self) -> &[String] {
        &self.ignores
    }

    pub fn build_output(&self) -> Option<&BuildOutputPlan> {
        self.build_output.as_ref()
    }

    fn from_legacy_lines(targets: Vec<BuildTarget>) -> Result<Self, String> {
        (!targets.is_empty())
            .then_some(Self {
                targets,
                ignores: vec![],
                build_output: None,
            })
            .ok_or_else(|| "mxrun config has no targets".to_string())
    }

    fn from_yaml(src: &str) -> Result<Self, String> {
        let config: YamlConfig =
            serde_yaml::from_str(src).map_err(|err| format!("invalid YAML mxrun config: {err}"))?;
        let targets = config
            .targets
            .iter()
            .enumerate()
            .map(|(index, target)| {
                Line {
                    lineno: index + 1,
                    text: target,
                }
                .parse()
                .map_err(|err| format!("invalid YAML target {}: {err}", index + 1))
            })
            .collect::<Result<Vec<_>, _>>()?;

        let build_output = config
            .project
            .build
            .map(|build| BuildOutputPlan::new(build.src, build.files, build.dst))
            .transpose()?;

        (!targets.is_empty())
            .then_some(Self {
                targets,
                ignores: config.project.ignore,
                build_output,
            })
            .ok_or_else(|| "mxrun config has no targets".to_string())
    }

    fn is_yaml(src: &str) -> bool {
        src.lines().map(str::trim).any(|line| {
            !line.is_empty() && !line.starts_with('#') && (line == "targets:" || line == "project:")
        })
    }
}

#[derive(Deserialize)]
struct YamlConfig {
    targets: Vec<String>,
    #[serde(default)]
    project: ProjectConfig,
}

#[derive(Default, Deserialize)]
struct ProjectConfig {
    #[serde(default, deserialize_with = "deserialize_string_list")]
    ignore: Vec<String>,
    build: Option<YamlBuildOutput>,
}

#[derive(Deserialize)]
struct YamlBuildOutput {
    #[serde(deserialize_with = "deserialize_string_list")]
    src: Vec<String>,
    #[serde(default, deserialize_with = "deserialize_optional_string_list")]
    files: Option<Vec<String>>,
    dst: String,
}

fn deserialize_string_list<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: Deserializer<'de>,
{
    string_values(Vec::<serde_yaml::Value>::deserialize(deserializer)?)
}

fn deserialize_optional_string_list<'de, D>(
    deserializer: D,
) -> Result<Option<Vec<String>>, D::Error>
where
    D: Deserializer<'de>,
{
    Option::<Vec<serde_yaml::Value>>::deserialize(deserializer)?
        .map(string_values)
        .transpose()
}

fn string_values<E>(values: Vec<serde_yaml::Value>) -> Result<Vec<String>, E>
where
    E: Error,
{
    values
        .into_iter()
        .map(|value| match value {
            serde_yaml::Value::String(pattern) => Ok(pattern),
            _ => Err(E::custom("entries must be strings")),
        })
        .collect()
}

struct Line<'a> {
    lineno: usize,
    text: &'a str,
}

impl<'a> Line<'a> {
    fn meaningful((lineno, raw): (usize, &'a str)) -> Option<Self> {
        let text = raw.trim();

        if text.is_empty() || text.starts_with('#') {
            None
        } else {
            Some(Self {
                lineno: lineno + 1,
                text,
            })
        }
    }

    fn parse(self) -> Result<BuildTarget, String> {
        if self.text == "local" {
            Ok(BuildTarget::local())
        } else {
            self.remote_target()
        }
    }

    fn remote_target(&self) -> Result<BuildTarget, String> {
        self.fields().and_then(|fields| {
            fields[2]
                .contains(':')
                .then_some(BuildTarget::remote(fields[0], fields[1], fields[2]))
                .ok_or_else(|| {
                    format!(
                        "invalid mxrun line {}: missing host:/destination in third field",
                        self.lineno
                    )
                })
        })
    }

    fn fields(&self) -> Result<Vec<&str>, String> {
        self.text
            .split_whitespace()
            .collect::<Vec<_>>()
            .pipe_ref(|fields| {
                (fields.len() == 3)
                    .then_some(fields.clone())
                    .ok_or_else(|| {
                        format!(
                            "invalid mxrun line {}: expected 3 fields, got {}",
                            self.lineno,
                            fields.len()
                        )
                    })
            })
    }
}

trait PipeRef: Sized {
    fn pipe_ref<T>(self, f: impl FnOnce(&Self) -> T) -> T {
        f(&self)
    }
}

impl<T> PipeRef for T {}
