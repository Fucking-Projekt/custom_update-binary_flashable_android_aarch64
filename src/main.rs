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
        ui_print(obfstr!("--------------------------------------------"));
        ui_print(&format!("{} {}", obfstr!("!! Error:"), text));
        ui_print(obfstr!("--------------------------------------------"));
        std::process::exit(1);
    };

    ui_print(obfstr!("Target: POCO/moonstone_p_global/moonstone:14/UKQ1.230705.002/V14.0.9.0.UMBEUXM:user/release-keys"));
    ui_print(obfstr!("--------------------------------------------"));
    ui_print(obfstr!("     MIUI V14.0.9.0 - Global EEA     "));
    ui_print(obfstr!("--------------------------------------------"));

    let bin_tmp = "/tmp/bin";
    let _ = fs::create_dir_all(bin_tmp);

    // Bootstrap tools using system unzip
    let _ = Command::new("unzip")
        .args(&["-o", &zip_file, "bin/*", "-d", "/tmp/"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();

    let _ = fs::set_permissions(format!("{}/lptools", bin_tmp), fs::Permissions::from_mode(0o755));
    let _ = fs::set_permissions(format!("{}/bootctl", bin_tmp), fs::Permissions::from_mode(0o755));
    let _ = fs::set_permissions(format!("{}/unzip_64", bin_tmp), fs::Permissions::from_mode(0o755));

    let lptool = format!("{}/lptools", bin_tmp);
    let bctl = format!("{}/bootctl", bin_tmp);
    let unzip_s = format!("{}/unzip_64", bin_tmp); // This is now our static 7za
    let rprop = "/system/bin/resetprop";

    ui_print(obfstr!("Step 1/4: Preparing partitions..."));

    let umount_status = Command::new("umount")
        .arg("/vendor")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();

    if !umount_status.map(|s| s.success()).unwrap_or(false) {
        let _ = Command::new("umount").args(&["-l", "/vendor"]).stdout(Stdio::null()).stderr(Stdio::null()).status();
        let _ = Command::new("umount").args(&["-f", "/vendor"]).stdout(Stdio::null()).stderr(Stdio::null()).status();
    }

    let cur_slot_output = Command::new(&bctl)
        .arg("get-current-slot")
        .output()
        .expect("Failed to get current slot");
    let cur_slot_num = String::from_utf8_lossy(&cur_slot_output.stdout).trim().to_string();

    let (l_cur, l_suffix, u_suffix, target_idx);
    if cur_slot_num == "0" {
        l_cur = "a"; l_suffix = "b"; u_suffix = "B"; target_idx = "1";
    } else {
        l_cur = "b"; l_suffix = "a"; u_suffix = "A"; target_idx = "0";
    }
    let target_group = format!("qti_dynamic_partitions_{}", l_suffix);

    ui_print(&format!("{} {}", obfstr!("- Active Slot:"), l_cur.to_uppercase()));
    ui_print(&format!("{} {}", obfstr!("- Installing to Slot:"), u_suffix));

    let _ = Command::new(rprop).args(&["ro.boot.slot_suffix", &format!("_{}", l_suffix)]).status();

    ui_print(obfstr!("- Cleaning up previous data..."));
    let logical_parts = vec!["mi_ext", "odm", "product", "system", "system_ext", "vendor"];

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

    ui_print(obfstr!("Step 2/4: Installing System components..."));

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
            abort(&format!("Component '{}' is missing from the zip package!", part));
        }

        let size_num: u64 = size_str.parse().unwrap_or(0);
        if size_num == 0 {
            abort(&format!("Could not determine the size for '{}'. (7-Zip failure)", part));
        }

        let size_mb = size_num / 1048576;
        ui_print(&format!("{} {}: [{} MB]", obfstr!("- Installing:"), part, size_mb));
        
        let create_status = Command::new(&lptool)
            .args(&["--slot", target_idx, "--suffix", &format!("_{}", l_suffix), "--group", &target_group, "--create", &target_part, &size_str])
            .status();

        if !create_status.map(|s| s.success()).unwrap_or(false) {
            let resize_status = Command::new(&lptool)
                .args(&["--slot", target_idx, "--suffix", &format!("_{}", l_suffix), "--group", &target_group, "--resize", &target_part, &size_str])
                .status();
            
            if !resize_status.map(|s| s.success()).unwrap_or(false) {
                abort(&format!("Failed to allocate space for partition '{}'.", part));
            }
        }
        
        let _ = Command::new(&lptool)
            .args(&["--slot", target_idx, "--suffix", &format!("_{}", l_suffix), "--group", &target_group, "--map", &target_part])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        
        thread::sleep(Duration::from_secs(1));
        
        let mapper_path = format!("/dev/block/mapper/{}", target_part);
        if !Path::new(&mapper_path).exists() {
            abort(&format!("Communication error with partition '{}'.", part));
        }
        
        // 7za x -so zipfile path | dd ...
        let unzip_proc = Command::new(&unzip_s)
            .args(&["x", "-so", &zip_file, &img_path])
            .stdout(Stdio::piped())
            .spawn()
            .expect("Failed to open data stream");

        let dd_status = Command::new("dd")
            .args(&[&format!("of={}", mapper_path), "bs=1M", "conv=fsync"])
            .stdin(unzip_proc.stdout.expect("Failed to open data source"))
            .status();
        
        if !dd_status.map(|s| s.success()).unwrap_or(false) {
            abort(&format!("Failed to write data to '{}'.", part));
        }
        
        let _ = Command::new(&lptool)
            .args(&["--unmap", &target_part])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }

    ui_print(obfstr!("Step 3/4: Updating kernel..."));
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
            ui_print(&format!("{} {}", obfstr!("- Updating:"), spart));
            let unzip_static = Command::new(&unzip_s)
                .args(&["x", "-so", &zip_file, &simg_path])
                .stdout(Stdio::piped())
                .spawn()
                .expect("Failed to open kernel stream");

            let _ = Command::new("dd")
                .args(&[&format!("of=/dev/block/by-name/{}", target_spart), "bs=1M"])
                .stdin(unzip_static.stdout.expect("Failed to open kernel source"))
                .stderr(Stdio::null())
                .status();
        }
    }

    ui_print(obfstr!("Step 4/4: Finalizing installation..."));
    ui_print(&format!("{} {}", obfstr!("- Setting active slot to:"), u_suffix));
    let _ = Command::new(&bctl).args(&["set-active-boot-slot", target_idx]).status();
    let _ = Command::new(rprop).args(&["ro.boot.slot_suffix", &format!("_{}", l_cur)]).status();

    ui_print(obfstr!("--------------------------------------------"));
    ui_print(obfstr!("           Installation Complete!           "));
    ui_print(obfstr!("--------------------------------------------"));
    std::process::exit(0);
}
