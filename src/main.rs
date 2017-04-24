extern crate libc;

const CHROOT_DIR: &'static str = "/home/vagrant/xenial";

use std::env;
use std::io::prelude::*;

use std::fs;
use std::fs::File;
use std::path::Path;
use std::process::Command;
use std::os::unix::process::CommandExt;
use std::os::unix::fs::PermissionsExt;

use std::ffi::CString;

fn main() {
  let cmd = env::args()
      .nth(1)
      .unwrap_or("help".to_string());

  match &*cmd {
     "run"   => { run() }
     "child" => { child() }
     _       => { help() }
   }
}

fn help() {
  println!("{}", "
usage: rust-containers <run|child> <args>

      run:            run a parent which invokes itself, double forking

    child:            run child process in container, invoking <args>

     args:            arguments to pass to the child
                      the parent will ignore any <args> that are supplied

");
}

fn run () {

  let bin = fs::read_link("/proc/self/exe").unwrap();

  let mut args: Vec<String> =
    env::args()
      .skip(2)
      .collect();

  args.insert(0, "child".to_string());

  println!("Parent Running: {:?} {:?}", bin, args);

  Command::new(bin)
      .args(&args)
      .before_exec(|| {
          unsafe {
            libc::unshare(libc::CLONE_NEWUTS | libc::CLONE_NEWPID | libc::CLONE_NEWNS);
          }
          Ok(())
      })
      .spawn()
      .unwrap()
      .wait()
      .unwrap();
}

fn child () {
  let args: Vec<String> = env::args()
      .skip(2)
      .collect();

  let (cmd, cmd_args) = args
    .split_first()
    .unwrap();

  cg();

  let hostname = "container";
  let chroot   = CString::new(CHROOT_DIR).unwrap();
  let rootdir  = CString::new("/").unwrap();
  let procfs   = CString::new("proc").unwrap();
  let tmpfs    = CString::new("tmpfs").unwrap();

  unsafe {
    println!("Child changing to hostname: {:?}", hostname);
    libc::sethostname(hostname.as_ptr() as *const i8, hostname.len());

    println!("Child chroot to directory: {:?}", chroot.to_str().unwrap());
    libc::chroot(chroot.as_ptr());

    println!("Child chdir to new root: {:?}", rootdir.to_str().unwrap());
    libc::chdir(rootdir.as_ptr());

    println!("Child mounting: {:?}", procfs.to_str().unwrap());
    libc::mount(procfs.as_ptr(), "/proc".as_ptr() as *const i8, procfs.as_ptr(), 0, std::ptr::null());

    println!("Child mounting: {:?}", tmpfs.to_str().unwrap());
    libc::mount(tmpfs.as_ptr(), "/tmp".as_ptr() as *const i8, tmpfs.as_ptr(), 0, std::ptr::null());
  }

  println!("Child Running: {:?} {:?}", cmd, cmd_args);

  Command::new(cmd)
      .args(cmd_args)
	    .spawn()
      .expect("Failed to spawn child")
      .wait()
      .unwrap();

  unsafe {
    libc::umount("/proc".as_ptr() as *const i8);
    libc::umount("/tmp".as_ptr() as *const i8);
  }
}

fn cg () {
  let cgroups = Path::new("/sys/fs/cgroup");
  let pids = cgroups.join("pids");

  {
    let dirname = pids.join("dirty");
    println!("touching {:?}", dirname);
    fs::create_dir_all(&dirname).unwrap();
    let mut perms = fs::metadata(&dirname).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&dirname, perms).ok();
  }

  {
    let filename = pids.join("dirty/pids.max");
    println!("touching {:?}", filename);
    let mut file = File::create(&filename).unwrap();
    file.write_all(b"20").ok();
    let mut perms = fs::metadata(&filename).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&filename, perms).ok();
  }

  {
    let filename = pids.join("dirty/notify_on_release");
    println!("touching {:?}", filename);
    let mut file = File::create(&filename).unwrap();
    file.write_all(b"1").ok();
    let mut perms = fs::metadata(&filename).unwrap().permissions();
    perms.set_mode(0o700);
    fs::set_permissions(&filename, perms).ok();
  }

  {
    let filename = pids.join("dirty/cgroup.procs");
    println!("touching {:?}", filename);

    unsafe {
      let pid = libc::getpid() as u8;
      let mut file = File::create(&filename).unwrap();
      file.write_all(&[pid]).ok();
    }

    let mut perms = fs::metadata(&filename).unwrap().permissions();
    perms.set_mode(0o700);
    fs::set_permissions(&filename, perms).ok();
  }
}
