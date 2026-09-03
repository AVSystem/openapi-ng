use std::path::{Path, PathBuf};

/// Working directory of the host process that invoked the generator.
/// Native builds ask the OS. WASI has no host cwd, but the Node loader
/// passes `process.env` through, so `PWD` carries it there.
#[cfg(not(target_os = "wasi"))]
pub(crate) fn host_cwd() -> Option<PathBuf> {
  std::env::current_dir().ok()
}

#[cfg(target_os = "wasi")]
pub(crate) fn host_cwd() -> Option<PathBuf> {
  std::env::var_os("PWD").map(PathBuf::from)
}

/// Join a relative path onto the host cwd; absolute paths pass through.
/// Without a known cwd the path is returned unchanged.
pub(crate) fn resolve_against_host_cwd(path: &Path) -> PathBuf {
  if path.is_absolute() {
    return path.to_path_buf();
  }
  host_cwd().map_or_else(|| path.to_path_buf(), |cwd| cwd.join(path))
}

#[cfg(test)]
mod tests {
  use super::{host_cwd, resolve_against_host_cwd};
  use std::path::{Path, PathBuf};

  #[test]
  fn absolute_paths_pass_through_unchanged() {
    let absolute = if cfg!(windows) {
      PathBuf::from(r"C:\specs\petstore.yaml")
    } else {
      PathBuf::from("/specs/petstore.yaml")
    };
    assert_eq!(resolve_against_host_cwd(&absolute), absolute);
  }

  #[test]
  fn relative_paths_join_the_host_cwd() {
    let cwd = host_cwd().expect("native builds always have a cwd");
    let relative = Path::new("test/fixtures/petstore.yaml");
    assert_eq!(resolve_against_host_cwd(relative), cwd.join(relative));
  }
}
