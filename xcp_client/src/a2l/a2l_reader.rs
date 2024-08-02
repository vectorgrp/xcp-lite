//-----------------------------------------------------------------------------
// Module a2l_reader
// Simplified A2L reader for integration testing
// Uses a2lfile crate to load A2L file

#![allow(dead_code)]

use std::{fs::File, io::Write};

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
    Signed,
    Unsigned,
    Float,
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

pub fn a2l_load(filename: &str) -> Result<a2lfile::A2lFile, a2lfile::A2lError> {
    trace!("Load A2L file {}", filename);
    let input_filename = &std::ffi::OsString::from(filename);
    let mut logmsgs = Vec::<A2lError>::new();
    let a2l_file = a2lfile::load(input_filename, None, &mut logmsgs, true)?;
    for log_msg in logmsgs {
        warn!("A2lLoader: {}", log_msg);
    }

    // Perform a consistency check
    let mut logmsgs = Vec::<String>::new();
    a2l_file.check(&mut logmsgs);

    Ok(a2l_file)
}

pub fn a2l_find_characteristic(
    a2l_file: &A2lFile,
    name: &str,
) -> Option<(A2lAddr, A2lType, A2lLimits)> {
    let o = a2l_file.project.module[0]
        .characteristic
        .iter()
        .find(|m| m.name == name);
    if o.is_none() {
        None
    } else {
        let c = o.unwrap();
        debug!("Found characteristic {}", c.name);
        let a2l_addr = c.address;
        let a2l_ext = c
            .ecu_address_extension
            .clone()
            .map(|e| e.extension)
            .unwrap_or_default();
        let characteristic_type = c.characteristic_type; //Ascii,Curve,Map,Cuboid,Cube4,Cube5,ValBlk,Value
        let deposit = c.deposit.clone(); // record layout name

        let conversion = c.conversion.clone();
        let a2l_lower_limit = c.lower_limit;
        let a2l_upper_limit = c.upper_limit;
        debug!(
            "addr: {}:{:08X} type: {:?} deposit :{:?} conversion: {:?} lower: {} upper: {}",
            a2l_ext,
            a2l_addr,
            characteristic_type,
            deposit,
            conversion,
            a2l_lower_limit,
            a2l_upper_limit
        );

        let a2l_size: u8;
        let a2l_encoding: A2lTypeEncoding;
        if deposit == "U8" || deposit == "S8" {
            a2l_size = 1;
            a2l_encoding = if deposit == "U8" {
                A2lTypeEncoding::Unsigned
            } else {
                A2lTypeEncoding::Signed
            };
        } else if deposit == "U16" || deposit == "S16" {
            a2l_size = 2;
            a2l_encoding = if deposit == "U16" {
                A2lTypeEncoding::Unsigned
            } else {
                A2lTypeEncoding::Signed
            };
        } else if deposit == "U32" || deposit == "S32" || deposit == "F32" {
            a2l_size = 4;
            a2l_encoding = if deposit == "U32" {
                A2lTypeEncoding::Unsigned
            } else if deposit == "S32" {
                A2lTypeEncoding::Signed
            } else {
                A2lTypeEncoding::Float
            };
        } else if deposit == "U64" || deposit == "S64" || deposit == "F64" {
            a2l_size = 8;
            a2l_encoding = if deposit == "U64" {
                A2lTypeEncoding::Unsigned
            } else if deposit == "S64" {
                A2lTypeEncoding::Signed
            } else {
                A2lTypeEncoding::Float
            };
        } else {
            return None;
        }

        Some((
            A2lAddr {
                ext: a2l_ext as u8,
                addr: a2l_addr,
                event: if a2l_ext == 0 {
                    (a2l_addr >> 16) as u16
                } else {
                    0
                },
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

pub fn a2l_find_measurement(a2l_file: &A2lFile, name: &str) -> Option<(A2lAddr, A2lType)> {
    let m = a2l_file.project.module[0]
        .measurement
        .iter()
        .find(|m| m.name == name)?;
    let a2l_addr: u32 = m
        .ecu_address
        .clone()
        .expect("measurement ecu_address not defined!")
        .address;
    let a2l_ext: i16 = m
        .ecu_address_extension
        .clone()
        .expect("ecu_address_extension not defined!")
        .extension;
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
    assert!(a2l_ext <= 0xFF);
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
            ext: a2l_ext as u8,
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
    info!("MOD_PAR:");
    if let Some(mod_par) = &a2l_file.project.module[0].mod_par {
        if let Some(epk) = &mod_par.epk {
            info!(" epk={}", epk.identifier);
        }
        for mem_seg in &mod_par.memory_segment {
            info!(
                " mem_seg {} {:0X}:{}",
                mem_seg.name, mem_seg.address, mem_seg.size
            );
            //info!(" if_data: {:?}", mem_seg.if_data);
        }
    }

    // MEASUREMENT
    info!("MEASUREMENT:");
    for measurement in &a2l_file.project.module[0].measurement {
        let addr = measurement
            .ecu_address
            .clone()
            .expect("ecu_address not defined!")
            .address;
        let ext = measurement
            .ecu_address_extension
            .clone()
            .expect("ecu_address_extentsion not defined!")
            .extension;
        info!(
            " {} {} {}:0x{:X}",
            measurement.name, measurement.datatype, ext, addr
        );
    }

    // CHARACTERISTIC
    info!("CHARACTERISTIC:");
    for characteristic in &a2l_file.project.module[0].characteristic {
        info!(
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

    let filename = "a2lfile.txt";
    let mut file = File::create(filename).expect("create file failed");
    let s = format!("{:#?}", a2l_file);
    file.write_all(s.as_bytes()).expect("write failed");

    // Write A2L to a file
    let filename = "a2lfile.a2l";
    a2l_file
        .write(
            std::ffi::OsString::from(filename),
            Some("Rewritten by xcp_lite"),
        )
        .expect("failed to write output");
}
