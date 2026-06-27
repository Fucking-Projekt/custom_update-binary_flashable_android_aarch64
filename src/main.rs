use std::env;
use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::process::{Command, Stdio};
use std::path::Path;
use std::thread;
use std::time::Duration;
use obfstr::obfstr;

fn main() {
    let args: Vec<String> = env::args().collect();

    let out_fd_str = args.get(2).cloned().unwrap_or_else(|| "1".to_string());
    let out_fd: i32 = out_fd_str.parse().unwrap_or(1);

    let mut zip_file = args.get(3).cloned().unwrap_or_default();

    if zip_file.is_empty() {
        for (key, value) in env::vars() {
            if key.to_lowercase().contains("zip") {
                zip_file = value;
                break;
            }
        }
    }

    if zip_file.is_empty() {
        let path = format!("/proc/self/fd/{}", out_fd);
        if let Ok(mut fd) = fs::OpenOptions::new().write(true).open(&path) {
            let _ = writeln!(fd, "ui_print !! Error: Installation package not found!");
        }
        std::process::exit(1);
    }

    let old_ld_path = env::var("LD_LIBRARY_PATH").unwrap_or_default();
    let new_ld_path = format!("/system/lib64:/system/lib:/vendor/lib64:/vendor/lib:{}", old_ld_path);
    env::set_var("LD_LIBRARY_PATH", &new_ld_path);

    let ui_print = |text: &str| {
        let path = format!("/proc/self/fd/{}", out_fd);
        if let Ok(mut fd) = fs::OpenOptions::new().write(true).open(&path) {
            let _ = writeln!(fd, "ui_print {}", text);
            let _ = writeln!(fd, "ui_print ");
        } else {
            println!("{}", text);
        }
    };

    let abort = |text: &str| {
        ui_print(obfstr!("============================================"));
        ui_print(&format!("{} {}", obfstr!("[FATAL ERROR]:"), text));
        ui_print(obfstr!("============================================"));
        std::process::exit(1);
    };

    ui_print(obfstr!("Target: Xiaomi/mivendor_sm6375_global/mivendor:11/RKQ1.230210.001/royan12240458:user/release-keys"));
    ui_print(obfstr!("--------------------------------------------"));
    ui_print(obfstr!("     Rom: HyperOS 3.0.306.0.WOACNXM.C09     "));
    ui_print(obfstr!("     From: Xiaomi 15 Ultra                  "));
    ui_print(obfstr!("     Android: 16                            "));
    ui_print(obfstr!("     Device: POCO X5 - Redmi Note 12 5G -   "));
    ui_print(obfstr!("     Redmi Note 12R Pro 5G (stone family)   "));
    ui_print(obfstr!("     Brought to you by: MyTeam              "));
    ui_print(obfstr!("--------------------------------------------"));

    let bin_tmp = "/tmp/bin";
    let _ = fs::create_dir_all(bin_tmp);

    // Bootstrap tools using system unzip
    let bootstrap_status = Command::new("unzip")
        .args(&["-o", &zip_file, "bin/*", "-d", "/tmp/"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();

    if !bootstrap_status.map(|s| s.success()).unwrap_or(false) {
        abort("Damn, couldn't extract installer binaries from the ZIP!");
    }

    for tool in &["lptools", "bootctl", "unzip_64"] {
        let tool_path = format!("{}/{}", bin_tmp, tool);
        if !Path::new(&tool_path).exists() {
            abort(&format!("Bruh, missing required tool '{}' in bootstrap!", tool));
        }
        if let Err(e) = fs::set_permissions(&tool_path, fs::Permissions::from_mode(0o755)) {
            abort(&format!("Failed to chmod '{}' (perms denied?): {}", tool, e));
        }
    }

    let lptool = format!("{}/lptools", bin_tmp);
    let bctl = format!("{}/bootctl", bin_tmp);
    let unzip_s = format!("{}/unzip_64", bin_tmp); // This is now our static 7za
    let rprop = "/system/bin/resetprop";

    ui_print(obfstr!("Step 1/4: Prepping partitions..."));

    let logical_parts = vec!["odm", "product", "system", "system_ext", "vendor", "mi_ext"];

    let unmount_partition = |mount_point: &str| {
        // Force lazy unmount (Toybox frees loop devices by default, -D prevents it)
        let _ = Command::new("umount")
            .args(&["-l", "-f", mount_point])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    };

    for part in &logical_parts {
        unmount_partition(&format!("/{}", part));
        if *part == "system" {
            unmount_partition("/system_root");
        }
    }

    let cur_slot_output = Command::new(&bctl)
        .arg("get-current-slot")
        .output();

    let cur_slot_res = match cur_slot_output {
        Ok(out) if out.status.success() => out,
        Ok(out) => {
            let err_msg = String::from_utf8_lossy(&out.stderr).trim().to_string();
            abort(&format!("bootctl failed to get current slot: {}", err_msg));
            std::process::exit(1);
        }
        Err(e) => {
            abort(&format!("Failed to execute bootctl: {}", e));
            std::process::exit(1);
        }
    };
    let cur_slot_num = String::from_utf8_lossy(&cur_slot_res.stdout).trim().to_string();

    let (l_cur, l_suffix, u_suffix, target_idx);
    if cur_slot_num == "0" {
        l_cur = "a"; l_suffix = "b"; u_suffix = "B"; target_idx = "1";
    } else {
        l_cur = "b"; l_suffix = "a"; u_suffix = "A"; target_idx = "0";
    }
    let target_group = format!("qti_dynamic_partitions_{}", l_suffix);

    ui_print(&format!("{} {}", obfstr!("- Current Slot:"), l_cur.to_uppercase()));
    ui_print(&format!("{} {}", obfstr!("- Target Slot:"), u_suffix));

    let _ = Command::new(rprop).args(&["ro.boot.slot_suffix", &format!("_{}", l_suffix)]).status();

    ui_print(obfstr!("- Nuking old dynamic partitions..."));

    for p in &logical_parts {
        let t_part = format!("{}_{}", p, l_suffix);
        let c_part = format!("{}_{}", p, l_cur);
        
        let _ = Command::new(&lptool).args(&["--slot", "0", "--unmap", &t_part]).stdout(Stdio::null()).stderr(Stdio::null()).status();
        let _ = Command::new(&lptool).args(&["--slot", "1", "--unmap", &t_part]).stdout(Stdio::null()).stderr(Stdio::null()).status();
        let _ = Command::new(&lptool).args(&["--slot", "0", "--unmap", &c_part]).stdout(Stdio::null()).stderr(Stdio::null()).status();
        let _ = Command::new(&lptool).args(&["--slot", "1", "--unmap", &c_part]).stdout(Stdio::null()).stderr(Stdio::null()).status();
        
        let _ = Command::new(&lptool).args(&["--slot", "0", "--remove", &t_part]).stdout(Stdio::null()).stderr(Stdio::null()).status();
        let _ = Command::new(&lptool).args(&["--slot", "1", "--remove", &t_part]).stdout(Stdio::null()).stderr(Stdio::null()).status();
        let _ = Command::new(&lptool).args(&["--slot", "0", "--remove", &c_part]).stdout(Stdio::null()).stderr(Stdio::null()).status();
        let _ = Command::new(&lptool).args(&["--slot", "1", "--remove", &c_part]).stdout(Stdio::null()).stderr(Stdio::null()).status();
    }

    let _ = Command::new(&lptool).args(&["--slot", "0", "--clear-cow"]).stdout(Stdio::null()).stderr(Stdio::null()).status();
    let _ = Command::new(&lptool).args(&["--slot", "1", "--clear-cow"]).stdout(Stdio::null()).stderr(Stdio::null()).status();

    ui_print(obfstr!("Step 2/4: Flashing logical partitions..."));

    for part in logical_parts {
        let target_part = format!("{}_{}", part, l_suffix);
        let mut img_path = String::new();
        let mut size_str = String::new();
        
        let paths_to_try = vec![
            format!("images/{}.img", part),
            format!("{}.img", part),
        ];

        for path in &paths_to_try {
            // Use 7za l -slt for robust size detection
            let output = Command::new(&unzip_s)
                .args(&["l", "-slt", &zip_file, path])
                .output();

            if let Ok(out) = output {
                let out_str = String::from_utf8_lossy(&out.stdout);
                for line in out_str.lines() {
                    if line.starts_with("Size =") {
                        let size = line.split('=').last().unwrap_or("0").trim();
                        if size != "0" {
                            size_str = size.to_string();
                            img_path = path.clone();
                            break;
                        }
                    }
                }
            }
            if !img_path.is_empty() { break; }
        }

        if img_path.is_empty() {
            if part == "mi_ext" {
                ui_print(&format!("- No '{}' image found, skipping (optional)...", part));
                continue;
            } else {
                abort(&format!("Whoops! '{}' image is missing from the zip!", part));
            }
        }

        let size_num: u64 = size_str.parse().unwrap_or(0);
        if size_num == 0 {
            abort(&format!("Can't read size for '{}' (Is your ZIP broken?)", part));
        }

        let size_mb = size_num / 1048576;
        ui_print(&format!("{} {}: [{} MB]", obfstr!("- Flashing"), part, size_mb));
        
        let create_status = Command::new(&lptool)
            .args(&["--slot", target_idx, "--suffix", &format!("_{}", l_suffix), "--group", &target_group, "--create", &target_part, &size_str])
            .status();

        if !create_status.map(|s| s.success()).unwrap_or(false) {
            let resize_status = Command::new(&lptool)
                .args(&["--slot", target_idx, "--suffix", &format!("_{}", l_suffix), "--group", &target_group, "--resize", &target_part, &size_str])
                .status();
            
            if !resize_status.map(|s| s.success()).unwrap_or(false) {
                abort(&format!("Not enough space in super partition for '{}'!", part));
            }
        }
        
        let _ = Command::new(&lptool)
            .args(&["--slot", target_idx, "--suffix", &format!("_{}", l_suffix), "--group", &target_group, "--map", &target_part])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        
        // We do not abort if --map returns non-zero, because some lptools binaries map during --create
        // or return non-zero if already mapped. We rely on the node_found check below.
        
        let mapper_path = format!("/dev/block/mapper/{}", target_part);
        let mut node_found = false;
        for _ in 0..50 { // 50 * 100ms = 5 seconds timeout
            if Path::new(&mapper_path).exists() {
                node_found = true;
                break;
            }
            thread::sleep(Duration::from_millis(100));
        }
        
        if !node_found {
            abort(&format!("Device node for '{}' didn't show up in time! udev issue?", part));
        }
        
        // 7za x -so zipfile path | dd ...
        let mut unzip_proc = Command::new(&unzip_s)
            .args(&["x", "-so", &zip_file, &img_path])
            .stdout(Stdio::piped())
            .spawn()
            .expect("Failed to open data stream");

        let unzip_stdout = unzip_proc.stdout.take().expect("Failed to open data source");

        let dd_status = Command::new("dd")
            .args(&[&format!("of={}", mapper_path), "bs=1M", "conv=fsync"])
            .stdin(unzip_stdout)
            .status();
        
        let unzip_status = unzip_proc.wait();
        
        let dd_ok = dd_status.map(|s| s.success()).unwrap_or(false);
        let unzip_ok = unzip_status.map(|s| s.success()).unwrap_or(false);

        if !dd_ok || !unzip_ok {
            abort(&format!("Flashing '{}' failed! (unzip: {}, dd: {})", part, unzip_ok, dd_ok));
        }
        
        let _ = Command::new(&lptool)
            .args(&["--unmap", &target_part])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }

    ui_print(obfstr!("Step 3/4: Flashing boot & kernel images..."));
    let static_parts = vec!["boot", "vendor_boot", "dtbo"];
    for spart in static_parts {
        let simg_path = format!("images/{}.img", spart);
        let target_spart = format!("{}_{}", spart, l_suffix);
        
        let unzip_l_output = Command::new(&unzip_s)
            .args(&["l", &zip_file, &simg_path])
            .output();

        let mut exists = false;
        if let Ok(output) = unzip_l_output {
            let output_str = String::from_utf8_lossy(&output.stdout);
            if output_str.contains(&simg_path) {
                exists = true;
            }
        }

        if exists {
            ui_print(&format!("{} {}", obfstr!("- Pushing:"), spart));
            let mut unzip_static = Command::new(&unzip_s)
                .args(&["x", "-so", &zip_file, &simg_path])
                .stdout(Stdio::piped())
                .spawn()
                .expect("Failed to open kernel stream");

            let unzip_static_stdout = unzip_static.stdout.take().expect("Failed to open kernel source");

            let dd_status = Command::new("dd")
                .args(&[&format!("of=/dev/block/by-name/{}", target_spart), "bs=1M"])
                .stdin(unzip_static_stdout)
                .status();

            let unzip_status = unzip_static.wait();

            let dd_ok = dd_status.map(|s| s.success()).unwrap_or(false);
            let unzip_ok = unzip_status.map(|s| s.success()).unwrap_or(false);

            if !dd_ok || !unzip_ok {
                abort(&format!("Kernel flash failed on '{}'! (unzip: {}, dd: {})", spart, unzip_ok, dd_ok));
            }
        }
    }

    ui_print(obfstr!("Step 4/4: Wrapping things up..."));
    ui_print(&format!("{} {}", obfstr!("- Switching active slot to:"), u_suffix));
    let set_slot_status = Command::new(&bctl).args(&["set-active-boot-slot", target_idx]).status();
    if !set_slot_status.map(|s| s.success()).unwrap_or(false) {
        abort("Failed to switch active boot slot! You might bootloop.");
    }
    let _ = Command::new(rprop).args(&["ro.boot.slot_suffix", &format!("_{}", l_cur)]).status();

    ui_print(obfstr!("============================================"));
    ui_print(obfstr!("        ROM successfully flashed! Enjoy     "));
    ui_print(obfstr!("============================================"));
    std::process::exit(0);
}
