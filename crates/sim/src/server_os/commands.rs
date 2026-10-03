use super::GuestFilesystem;
use crate::{DeviceId, NetworkSim};

pub(super) struct LinuxFiles;

impl LinuxFiles {
    pub fn execute(
        sim: &mut NetworkSim,
        device: DeviceId,
        command: &str,
        args: &[String],
        stdin: &str,
    ) -> Result<Vec<String>, String> {
        let cwd = sim.guest_mut(device).cwd.clone();
        let paths: Vec<_> = args
            .iter()
            .filter(|arg| !arg.starts_with('-'))
            .map(|arg| GuestFilesystem::normalize(&cwd, arg))
            .collect();
        match command {
            "pwd" => Ok(vec![cwd]),
            "cd" => {
                let path =
                    GuestFilesystem::normalize(&cwd, args.first().map_or("/root", String::as_str));
                if !sim
                    .guest_mut(device)
                    .filesystem
                    .entries
                    .get(&path)
                    .is_some_and(|file| file.directory)
                {
                    return Err(format!("cd: {path}: No such directory"));
                }
                sim.guest_mut(device).cwd = path;
                Ok(Vec::new())
            }
            "ls" => {
                let long = args
                    .iter()
                    .any(|arg| arg.starts_with('-') && arg.contains('l'));
                let all = args
                    .iter()
                    .any(|arg| arg.starts_with('-') && arg.contains('a'));
                let mut lines = Vec::new();
                for path in if paths.is_empty() { vec![cwd] } else { paths } {
                    for (name, file) in sim.guest_mut(device).filesystem.list(&path)? {
                        if !all && name.starts_with('.') {
                            continue;
                        }
                        lines.push(if long {
                            format!(
                                "{}{:03o} root root {:>8} {name}",
                                if file.directory { "d" } else { "-" },
                                file.mode,
                                file.contents.len()
                            )
                        } else {
                            format!("{name}{}", if file.directory { "/" } else { "" })
                        });
                    }
                }
                Ok(lines)
            }
            "mkdir" => {
                let parents = args.iter().any(|arg| arg == "-p");
                if paths.is_empty() {
                    return Err("mkdir: missing operand".into());
                }
                for path in paths {
                    sim.guest_mut(device).filesystem.mkdir(&path, parents)?;
                }
                Ok(Vec::new())
            }
            "touch" => {
                if paths.is_empty() {
                    return Err("touch: missing operand".into());
                }
                for path in paths {
                    if !sim.guest_mut(device).filesystem.entries.contains_key(&path) {
                        sim.guest_mut(device).filesystem.write(&path, "", false)?;
                    }
                }
                Ok(Vec::new())
            }
            "rm" | "rmdir" => {
                let recursive = args
                    .iter()
                    .any(|arg| arg.starts_with('-') && (arg.contains('r') || arg.contains('R')));
                let force = args
                    .iter()
                    .any(|arg| arg.starts_with('-') && arg.contains('f'));
                if paths.is_empty() {
                    return if force {
                        Ok(Vec::new())
                    } else {
                        Err("rm: missing operand".into())
                    };
                }
                for path in paths {
                    if command == "rmdir" {
                        let entries = sim.guest_mut(device).filesystem.list(&path)?;
                        if !entries.is_empty() {
                            return Err(format!("rmdir: {path}: Directory not empty"));
                        }
                    }
                    sim.guest_mut(device).filesystem.remove(
                        &path,
                        recursive || command == "rmdir",
                        force,
                    )?;
                }
                Ok(Vec::new())
            }
            "cp" | "mv" => {
                if paths.len() != 2 {
                    return Err(format!("usage: {command} SOURCE DESTINATION"));
                }
                let source = &paths[0];
                let mut destination = paths[1].clone();
                if sim
                    .guest_mut(device)
                    .filesystem
                    .entries
                    .get(&destination)
                    .is_some_and(|file| file.directory)
                {
                    destination = format!("{destination}/{}", source.rsplit('/').next().unwrap());
                }
                let contents = sim.guest_mut(device).filesystem.read(source)?;
                sim.guest_mut(device)
                    .filesystem
                    .write(&destination, &contents, false)?;
                if command == "mv" {
                    sim.guest_mut(device)
                        .filesystem
                        .remove(source, false, false)?;
                }
                Ok(Vec::new())
            }
            "chmod" => {
                if args.len() < 2 {
                    return Err("chmod: missing operand".into());
                }
                let mode = u16::from_str_radix(args.first().ok_or("chmod: missing mode")?, 8)
                    .map_err(|_| "chmod: use an octal mode such as 644")?;
                if mode > 0o7777 {
                    return Err("chmod: invalid mode".into());
                }
                for path in args
                    .iter()
                    .skip(1)
                    .map(|path| GuestFilesystem::normalize(&cwd, path))
                {
                    sim.guest_mut(device)
                        .filesystem
                        .entries
                        .get_mut(&path)
                        .ok_or_else(|| format!("{path}: No such file"))?
                        .mode = mode;
                }
                Ok(Vec::new())
            }
            "cat" | "head" | "tail" | "wc" | "grep" | "sort" | "uniq" | "tee" => {
                Self::text(sim, device, command, args, stdin)
            }
            "find" => {
                let root =
                    GuestFilesystem::normalize(&cwd, args.first().map_or(".", String::as_str));
                let pattern = args
                    .windows(2)
                    .find(|pair| pair[0] == "-name")
                    .map(|pair| pair[1].as_str());
                Ok(sim
                    .guest_mut(device)
                    .filesystem
                    .entries
                    .keys()
                    .filter(|path| **path == root || path.starts_with(&format!("{root}/")))
                    .filter(|path| {
                        pattern.is_none_or(|pattern| {
                            Self::matches(path.rsplit('/').next().unwrap_or(path), pattern)
                        })
                    })
                    .cloned()
                    .collect())
            }
            "df" => {
                let capacity = Self::capacity_gb(sim, device);
                let used = sim
                    .guest_mut(device)
                    .filesystem
                    .entries
                    .values()
                    .map(|file| file.contents.len())
                    .sum::<usize>();
                Ok(vec![
                    "Filesystem      Size     Used Available Mounted on".into(),
                    format!("guest-root      {capacity}G     {used}B     {capacity}G /"),
                    "tmpfs           64M        0B        64M /tmp".into(),
                ])
            }
            "du" => {
                let path = paths.first().unwrap_or(&cwd);
                let bytes: usize = sim
                    .guest_mut(device)
                    .filesystem
                    .entries
                    .iter()
                    .filter(|(name, _)| **name == *path || name.starts_with(&format!("{path}/")))
                    .map(|(_, file)| file.contents.len())
                    .sum();
                Ok(vec![format!("{}\t{path}", bytes.div_ceil(1024))])
            }
            "mount" => {
                if args.is_empty() {
                    Ok(vec![
                        "guest-root on / type simulatedfs (rw)".into(),
                        "proc on /proc type proc (ro)".into(),
                    ])
                } else {
                    Err("mount: no additional filesystem devices are available".into())
                }
            }
            _ => Err(format!("{command}: command not found")),
        }
    }

    fn capacity_gb(sim: &NetworkSim, device: DeviceId) -> u64 {
        let Some(crate::DeviceKind::Server(server)) = sim.device(device).map(|device| &device.kind)
        else {
            return 0;
        };
        server.hardware.as_ref().map_or(16, |hardware| {
            hardware
                .drives
                .iter()
                .flatten()
                .filter_map(|id| {
                    crate::drive_catalog()
                        .drives
                        .iter()
                        .find(|drive| drive.id == *id)
                })
                .map(|drive| u64::from(drive.capacity_gb))
                .sum::<u64>()
                .max(1)
        })
    }

    pub fn matches(value: &str, pattern: &str) -> bool {
        if pattern == "*" {
            return true;
        }
        if let Some((prefix, suffix)) = pattern.split_once('*') {
            return value.starts_with(prefix) && value.ends_with(suffix);
        }
        value == pattern
    }

    fn text(
        sim: &mut NetworkSim,
        device: DeviceId,
        command: &str,
        args: &[String],
        stdin: &str,
    ) -> Result<Vec<String>, String> {
        let cwd = sim.guest_mut(device).cwd.clone();
        if command == "tee" {
            for path in args.iter().filter(|arg| !arg.starts_with('-')) {
                sim.guest_mut(device).filesystem.write(
                    &GuestFilesystem::normalize(&cwd, path),
                    stdin,
                    args.iter().any(|arg| arg == "-a"),
                )?;
            }
            return Ok(stdin.lines().map(str::to_owned).collect());
        }
        let mut options = Vec::new();
        let mut files = Vec::new();
        let mut count = 10usize;
        let mut pattern = None;
        let mut index = 0;
        while index < args.len() {
            let arg = &args[index];
            if arg == "-n" && matches!(command, "head" | "tail") {
                index += 1;
                count = args
                    .get(index)
                    .ok_or("missing line count")?
                    .parse()
                    .map_err(|_| "invalid line count")?;
            } else if arg.starts_with('-') {
                options.push(arg.as_str());
            } else if command == "grep" && pattern.is_none() {
                pattern = Some(arg.as_str());
            } else {
                files.push(arg.as_str());
            }
            index += 1;
        }
        let mut contents = String::new();
        if files.is_empty() {
            contents.push_str(stdin);
        } else {
            for file in files {
                contents.push_str(
                    &sim.guest_mut(device)
                        .filesystem
                        .read(&GuestFilesystem::normalize(&cwd, file))?,
                );
            }
        }
        let mut lines: Vec<String> = contents.lines().map(str::to_owned).collect();
        match command {
            "cat" => {
                if options.iter().any(|option| option.contains('n')) {
                    lines = lines
                        .into_iter()
                        .enumerate()
                        .map(|(index, line)| format!("{:>6}\t{line}", index + 1))
                        .collect();
                }
            }
            "head" => lines.truncate(count),
            "tail" => {
                lines = lines.into_iter().rev().take(count).collect();
                lines.reverse();
            }
            "grep" => {
                let pattern = pattern.ok_or("grep: missing pattern")?;
                let insensitive = options.iter().any(|option| option.contains('i'));
                let invert = options.iter().any(|option| option.contains('v'));
                let numbered = options.iter().any(|option| option.contains('n'));
                lines = lines
                    .into_iter()
                    .enumerate()
                    .filter(|(_, line)| {
                        let found = if insensitive {
                            line.to_lowercase().contains(&pattern.to_lowercase())
                        } else {
                            line.contains(pattern)
                        };
                        found != invert
                    })
                    .map(|(index, line)| {
                        if numbered {
                            format!("{}:{line}", index + 1)
                        } else {
                            line
                        }
                    })
                    .collect();
                if lines.is_empty() {
                    return Err(String::new());
                }
            }
            "sort" => {
                lines.sort();
                if options.contains(&"-r") {
                    lines.reverse();
                }
                if options.contains(&"-u") {
                    lines.dedup();
                }
            }
            "uniq" => lines.dedup(),
            "wc" => {
                let count = if options.contains(&"-l") {
                    contents
                        .bytes()
                        .filter(|byte| *byte == b'\n')
                        .count()
                        .to_string()
                } else if options.contains(&"-w") {
                    contents.split_whitespace().count().to_string()
                } else if options.contains(&"-c") {
                    contents.len().to_string()
                } else {
                    format!(
                        "{} {} {}",
                        contents.bytes().filter(|byte| *byte == b'\n').count(),
                        contents.split_whitespace().count(),
                        contents.len()
                    )
                };
                lines = vec![count];
            }
            _ => {}
        }
        Ok(lines)
    }
}
