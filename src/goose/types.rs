#![allow(non_snake_case)]
#![allow(non_camel_case_types)]
use serde::{Serialize, Deserialize};
use log::info;

#[derive(Debug,Serialize, Deserialize,Clone)]
pub enum IECData{
    array(Vec<IECData>),
    structure(Vec<IECData>),
    boolean(bool),

    int8(i8),
    int16(i16),
    int32(i32),
    int64(i64),

    int8u(u8),
    int16u(u16),
    int32u(u32),

    float32(f32),
    float64(f64),

    visible_string(String),
    mms_string(String),
    bit_string{ padding: u8, val: Vec<u8> },
    octet_string(Vec<u8>),
    utc_time([u8;8])
}

impl IECData {
    /// Cast to array, returns None if not an array
    pub fn as_array(&self) -> Option<&Vec<IECData>> {
        match self {
            IECData::array(v) => Some(v),
            _ => None,
        }
    }

    /// Cast to mutable array, returns None if not an array
    pub fn as_array_mut(&mut self) -> Option<&mut Vec<IECData>> {
        match self {
            IECData::array(v) => Some(v),
            _ => None,
        }
    }

    /// Cast to structure, returns None if not a structure
    pub fn as_structure(&self) -> Option<&Vec<IECData>> {
        match self {
            IECData::structure(v) => Some(v),
            _ => None,
        }
    }

    /// Cast to mutable structure, returns None if not a structure
    pub fn as_structure_mut(&mut self) -> Option<&mut Vec<IECData>> {
        match self {
            IECData::structure(v) => Some(v),
            _ => None,
        }
    }

    /// Cast to boolean, returns None if not a boolean
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            IECData::boolean(b) => Some(*b),
            _ => None,
        }
    }

    /// Cast to i8, returns None if not an int8
    pub fn as_i8(&self) -> Option<i8> {
        match self {
            IECData::int8(v) => Some(*v),
            _ => None,
        }
    }

    /// Cast to i16, returns None if not an int16
    pub fn as_i16(&self) -> Option<i16> {
        match self {
            IECData::int16(v) => Some(*v),
            _ => None,
        }
    }

    /// Cast to i32, returns None if not an int32
    pub fn as_i32(&self) -> Option<i32> {
        match self {
            IECData::int32(v) => Some(*v),
            _ => None,
        }
    }

    /// Cast to i64, returns None if not an int64
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            IECData::int64(v) => Some(*v),
            _ => None,
        }
    }

    /// Cast to u8, returns None if not an int8u
    pub fn as_u8(&self) -> Option<u8> {
        match self {
            IECData::int8u(v) => Some(*v),
            _ => None,
        }
    }

    /// Cast to u16, returns None if not an int16u
    pub fn as_u16(&self) -> Option<u16> {
        match self {
            IECData::int16u(v) => Some(*v),
            _ => None,
        }
    }

    /// Cast to u32, returns None if not an int32u
    pub fn as_u32(&self) -> Option<u32> {
        match self {
            IECData::int32u(v) => Some(*v),
            _ => None,
        }
    }

    /// Cast to f32, returns None if not a float32
    pub fn as_f32(&self) -> Option<f32> {
        match self {
            IECData::float32(v) => Some(*v),
            _ => None,
        }
    }

    /// Cast to f64, returns None if not a float64
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            IECData::float64(v) => Some(*v),
            _ => None,
        }
    }

    /// Cast to String reference (visible_string), returns None if not a visible_string
    pub fn as_visible_string(&self) -> Option<&String> {
        match self {
            IECData::visible_string(s) => Some(s),
            _ => None,
        }
    }

    /// Cast to String reference (mms_string), returns None if not an mms_string
    pub fn as_mms_string(&self) -> Option<&String> {
        match self {
            IECData::mms_string(s) => Some(s),
            _ => None,
        }
    }

    /// Cast to bit_string, returns None if not a bit_string
    /// Returns (padding, bit_values)
    pub fn as_bit_string(&self) -> Option<(u8, &Vec<u8>)> {
        match self {
            IECData::bit_string { padding, val } => Some((*padding, val)),
            _ => None,
        }
    }

    /// Cast to octet_string, returns None if not an octet_string
    pub fn as_octet_string(&self) -> Option<&Vec<u8>> {
        match self {
            IECData::octet_string(v) => Some(v),
            _ => None,
        }
    }

    /// Cast to utc_time, returns None if not a utc_time
    pub fn as_utc_time(&self) -> Option<&[u8; 8]> {
        match self {
            IECData::utc_time(t) => Some(t),
            _ => None,
        }
    }

    /// Check if this is an array variant
    pub fn is_array(&self) -> bool {
        matches!(self, IECData::array(_))
    }

    /// Check if this is a structure variant
    pub fn is_structure(&self) -> bool {
        matches!(self, IECData::structure(_))
    }

    /// Check if this is a boolean variant
    pub fn is_bool(&self) -> bool {
        matches!(self, IECData::boolean(_))
    }

    /// Check if this is any integer variant (signed or unsigned)
    pub fn is_integer(&self) -> bool {
        matches!(
            self,
            IECData::int8(_)
                | IECData::int16(_)
                | IECData::int32(_)
                | IECData::int64(_)
                | IECData::int8u(_)
                | IECData::int16u(_)
                | IECData::int32u(_)
        )
    }

    /// Check if this is any float variant
    pub fn is_float(&self) -> bool {
        matches!(self, IECData::float32(_) | IECData::float64(_))
    }

    /// Check if this is any string variant
    pub fn is_string(&self) -> bool {
        matches!(self, IECData::visible_string(_) | IECData::mms_string(_))
    }

    /// Check if this is a bit_string variant
    pub fn is_bit_string(&self) -> bool {
        matches!(self, IECData::bit_string { .. })
    }

    /// Check if this is an octet_string variant
    pub fn is_octet_string(&self) -> bool {
        matches!(self, IECData::octet_string(_))
    }

    /// Check if this is a utc_time variant
    pub fn is_utc_time(&self) -> bool {
        matches!(self, IECData::utc_time(_))
    }

    /// Get the variant name as a string for debugging/logging
    pub fn variant_name(&self) -> &'static str {
        match self {
            IECData::array(_) => "array",
            IECData::structure(_) => "structure",
            IECData::boolean(_) => "boolean",
            IECData::int8(_) => "int8",
            IECData::int16(_) => "int16",
            IECData::int32(_) => "int32",
            IECData::int64(_) => "int64",
            IECData::int8u(_) => "int8u",
            IECData::int16u(_) => "int16u",
            IECData::int32u(_) => "int32u",
            IECData::float32(_) => "float32",
            IECData::float64(_) => "float64",
            IECData::visible_string(_) => "visible_string",
            IECData::mms_string(_) => "mms_string",
            IECData::bit_string { .. } => "bit_string",
            IECData::octet_string(_) => "octet_string",
            IECData::utc_time(_) => "utc_time",
        }
    }
}

#[derive(Debug,Default)]
pub struct EthernetHeader {
    pub srcAddr:[u8;6],
    pub dstAddr:[u8;6],
    pub TPID:[u8;2],
    pub TCI:[u8;2],
    pub ehterType:[u8;2],
    pub APPID:[u8;2],
    pub length:[u8;2]
}

#[derive(Debug,Default,Clone)]
pub struct IECGoosePdu {
    pub gocbRef: String,
    pub timeAllowedtoLive: u32,
    pub datSet: String,
    pub goID: String,
    pub t: [u8;8],
    pub stNum: u32,
    pub sqNum: u32,
    pub simulation: bool,
    pub confRev: u32,
    pub ndsCom: bool,
    pub numDatSetEntries: u32,
    pub allData: Vec<IECData>
}

impl IECGoosePdu {
    pub fn report(&mut self) {
        info!("gocbRef:{},data:{:?}",self.gocbRef,self.allData);
    }
}

