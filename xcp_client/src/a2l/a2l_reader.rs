//-----------------------------------------------------------------------------
// Module a2l_reader
// Simplified A2L reader for integration testing
// Uses a2lfile crate to load A2L file

#![allow(dead_code)]

#[allow(unused_imports)]
use log::{debug, error, info, trace, warn};

use a2lfile::*;
use a2lfile::{A2lError, A2lFile};

use super::ifdata;

#[derive(Debug, Clone, Copy)]
pub struct A2lAddr {
    pub ext: u8,
    pub addr: u32,
    pub event: u16,
}

#[derive(Debug, Clone, Copy)]
pub enum A2lTypeEncoding {
    Signed = -1,
    Unsigned = 1,
    Float = 0,
}

#[derive(Debug, Clone, Copy)]
pub struct A2lType {
    pub size: u8,
    pub encoding: A2lTypeEncoding,
}

#[derive(Debug, Clone, Copy)]
pub struct A2lLimits {
    pub lower: f64,
    pub upper: f64,
}

pub fn a2l_load<P: AsRef<std::path::Path>>(filename: P) -> Result<a2lfile::A2lFile, a2lfile::A2lError> {
    let filename = filename.as_ref();
    trace!("Load A2L file {}", filename.display());
    let mut logmsgs = Vec::<A2lError>::new();
    let res = a2lfile::load(filename, None, &mut logmsgs, true);
    for log_msg in logmsgs {
        warn!("A2l Loader: {}", log_msg);
    }
    match res {
        Ok(a2l_file) => {
            // Perform a consistency check
            let mut logmsgs = Vec::<String>::new();
            a2l_file.check(&mut logmsgs);
            for log_msg in logmsgs {
                warn!("A2l Checker: {}", log_msg);
            }
            Ok(a2l_file)
        }

        Err(e) => {
            error!("a2lfile::load failed: {:?}", e);
            Err(e)
        }
    }
}

pub fn a2l_get_characteristics(a2l_file: &A2lFile) -> Vec<String> {
    let mut v = Vec::<String>::with_capacity(a2l_file.project.module[0].characteristic.len());
    for c in a2l_file.project.module[0].characteristic.iter() {
        v.push(c.name.clone());
    }
    v
}

pub fn a2l_find_characteristic(a2l_file: &A2lFile, name: &str) -> Option<(A2lAddr, A2lType, A2lLimits)> {
    let o = a2l_file.project.module[0].characteristic.iter().find(|m| m.name == name);
    if o.is_none() {
        debug!("Characteristic {} not found", name);
        None
    } else {
        let c = o.unwrap();
        let a2l_addr = c.address;
        let a2l_ext = c.ecu_address_extension.clone().map(|e| e.extension).unwrap_or_default();
        let characteristic_type = c.characteristic_type; //Ascii,Curve,Map,Cuboid,Cube4,Cube5,ValBlk,Value
        let conversion = c.conversion.clone();
        let a2l_lower_limit = c.lower_limit;
        let a2l_upper_limit = c.upper_limit;
        debug!(
            "Characteristic {}: addr: {}:{:08X} type: {:?} deposit :{:?} conversion: {:?} lower: {} upper: {}",
            c.name, a2l_ext, a2l_addr, characteristic_type, c.deposit, conversion, a2l_lower_limit, a2l_upper_limit
        );

        // Record layout
        // Hardcode xcp-lite and XCPlite names
        let a2l_size: u8;
        let a2l_encoding: A2lTypeEncoding;
        match c.deposit.as_str() {
            "U8" | "R_UBYTE" => {
                a2l_size = 1;
                a2l_encoding = A2lTypeEncoding::Unsigned;
            }
            "S8" | "R_SBYTE" => {
                a2l_size = 1;
                a2l_encoding = A2lTypeEncoding::Signed;
            }
            "U16" | "R_UWORD" => {
                a2l_size = 2;
                a2l_encoding = A2lTypeEncoding::Unsigned;
            }
            "S16" | "R_SWORD" => {
                a2l_size = 2;
                a2l_encoding = A2lTypeEncoding::Signed;
            }
            "U32" | "R_ULONG" => {
                a2l_size = 4;
                a2l_encoding = A2lTypeEncoding::Unsigned;
            }
            "S32" | "R_SLONG" => {
                a2l_size = 4;
                a2l_encoding = A2lTypeEncoding::Signed;
            }
            "U64" | "R_A_UINT64" | "R_ULONGLONG" => {
                a2l_size = 8;
                a2l_encoding = A2lTypeEncoding::Unsigned;
            }
            "S64" | "R_A_INT64" | "R_SLONGLONG" => {
                a2l_size = 8;
                a2l_encoding = A2lTypeEncoding::Signed;
            }
            "F32" | "R_FLOAT32_IEEE" => {
                a2l_size = 4;
                a2l_encoding = A2lTypeEncoding::Float;
            }
            "F64" | "R_FLOAT64_IEEE" => {
                a2l_size = 8;
                a2l_encoding = A2lTypeEncoding::Float;
            }
            _ => {
                warn!("Unknown deposit type {}", c.deposit);
                return None;
            }
        }

        Some((
            A2lAddr {
                ext: a2l_ext.try_into().expect("Address extension too large"),
                addr: a2l_addr,
                event: if a2l_ext == 0 { (a2l_addr >> 16) as u16 } else { 0 },
            },
            A2lType {
                size: a2l_size,
                encoding: a2l_encoding,
            },
            A2lLimits {
                lower: a2l_lower_limit,
                upper: a2l_upper_limit,
            },
        ))
    }
}

pub fn a2l_get_measurements(a2l_file: &A2lFile) -> Vec<String> {
    let mut v = Vec::<String>::with_capacity(a2l_file.project.module[0].measurement.len());
    for m in a2l_file.project.module[0].measurement.iter() {
        v.push(m.name.clone());
    }
    v
}

pub fn a2l_find_measurement(a2l_file: &A2lFile, name: &str) -> Option<(A2lAddr, A2lType)> {
    let m = a2l_file.project.module[0].measurement.iter().find(|m| m.name == name)?;
    let a2l_addr: u32 = m.ecu_address.clone().expect("Measurement ecu_address not found!").address;
    let a2l_ext: u8 = if let Some(e) = m.ecu_address_extension.clone() { e.extension } else { 0 }.try_into().unwrap();

    let get_type = m.datatype;
    let a2l_size: u8 = match get_type {
        DataType::Sbyte => 1,
        DataType::Sword => 2,
        DataType::Slong => 4,
        DataType::AInt64 => 8,
        DataType::Ubyte => 1,
        DataType::Uword => 2,
        DataType::Ulong => 4,
        DataType::AUint64 => 8,
        DataType::Float64Ieee => 8,
        DataType::Float32Ieee => 4,
        DataType::Float16Ieee => 2,
    };
    let a2l_encoding: A2lTypeEncoding = match get_type {
        DataType::Sbyte => A2lTypeEncoding::Signed,
        DataType::Sword => A2lTypeEncoding::Signed,
        DataType::Slong => A2lTypeEncoding::Signed,
        DataType::AInt64 => A2lTypeEncoding::Signed,
        DataType::Ubyte => A2lTypeEncoding::Unsigned,
        DataType::Uword => A2lTypeEncoding::Unsigned,
        DataType::Ulong => A2lTypeEncoding::Unsigned,
        DataType::AUint64 => A2lTypeEncoding::Unsigned,
        DataType::Float64Ieee => A2lTypeEncoding::Float,
        DataType::Float32Ieee => A2lTypeEncoding::Float,
        DataType::Float16Ieee => A2lTypeEncoding::Float,
    };
    assert!(a2l_size > 0, "a2l_size is zero");

    let mut a2l_event: u16 = 0xFFFF;
    let ifdata_vec = m.if_data.clone();

    for ifdata in &ifdata_vec {
        // println!("if_data: {:#?}", if_data);
        let decoded_ifdata = ifdata::A2mlVector::load_from_ifdata(ifdata).unwrap();
        //println!("decoded_ifdata: {:#?}", decoded_ifdata);
        if let Some(xcp) = decoded_ifdata.xcp {
            //println!("xcp: {:#?}", xcp);
            if let Some(daq_event) = xcp.daq_event {
                //println!("daq_event: {:#?}", daq_event);
                if let Some(fixed_event_list) = daq_event.fixed_event_list {
                    //println!("fixed_event_list: {:#?}", fixed_event_list);
                    a2l_event = fixed_event_list.event[0].item;
                    //println!("event =  {:#?}", a2l_event)
                }
            }
        }
    }
    assert_ne!(a2l_event, 0xFFFF, "IF_DATA fixed event number not found");

    Some((
        A2lAddr {
            ext: a2l_ext,
            addr: a2l_addr,
            event: a2l_event,
        },
        A2lType {
            size: a2l_size,
            encoding: a2l_encoding,
        },
    ))
}

pub fn a2l_printf_info(a2l_file: &A2lFile) {
    // MOD_PAR
    println!("MOD_PAR:");
    if let Some(mod_par) = &a2l_file.project.module[0].mod_par {
        if let Some(epk) = &mod_par.epk {
            println!(" epk = {}", epk.identifier);
        }
        for mem_seg in &mod_par.memory_segment {
            println!(" memory segment {} {:0X}:{}", mem_seg.name, mem_seg.address, mem_seg.size);
            //info!(" if_data: {:?}", mem_seg.if_data);
        }
    }

    // MEASUREMENT
    println!("MEASUREMENTS:");
    for measurement in &a2l_file.project.module[0].measurement {
        let addr = measurement.ecu_address.clone().expect("ecu_address not found!").address;
        let ext: u8 = if let Some(e) = measurement.ecu_address_extension.clone() { e.extension } else { 0 }.try_into().unwrap();
        println!(" {} {} {}:0x{:X}", measurement.name, measurement.datatype, ext, addr);
    }

    // CHARACTERISTIC
    println!("CHARACTERISTICS:");
    for characteristic in &a2l_file.project.module[0].characteristic {
        println!(
            " {} {:?} {} 0x{:X} {:?} {:?} {} {}",
            characteristic.name,
            characteristic.long_identifier,
            characteristic.deposit,
            characteristic.address,
            characteristic.characteristic_type,
            characteristic.conversion,
            characteristic.lower_limit,
            characteristic.upper_limit
        );
    }

    // Write A2L to a file
    // let filename = "a2lfile.txt";
    // let mut file = File::create(filename).expect("create file failed");
    // let s = format!("{:#?}", a2l_file);
    // file.write_all(s.as_bytes()).expect("write failed");
    // let filename = "a2lfile.a2l";
    // a2l_file.write(std::ffi::OsString::from(filename), Some("Rewritten by xcp-lite")).expect("failed to write output");
}
