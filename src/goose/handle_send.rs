use crate::goose::types::{EthernetHeader, IECData, IECGoosePdu};
use crate::goose::pdu::{encodeGooseFrame, getTimeMs};
use pnet_datalink::DataLinkSender;
use libc::sched_getcpu;
use std::thread;
use crate::os::linux_rt::pin_thread_to_core;
use log::{error, info};

const GOOSE_BUFFER_SIZE: usize = 512;

pub fn handle_send(mut tx: Box<dyn DataLinkSender>, num_workers: usize) {
    thread::spawn(move || {
        // Pin main thread to the last core
        if let Err(e) = pin_thread_to_core(num_workers - 1) {
            error!("Failed to pin send thread to core: {}", e);
        } else {
            info!("Send thread pinned to CPU: {}", unsafe { sched_getcpu() });
        }
        let mut ether_header = EthernetHeader {
            srcAddr: [0xe8, 0xd8, 0xd1, 0xeb, 0xcb, 0xb6],
            dstAddr: [0x01, 0x0C, 0xCD, 0x01, 0x00, 0x08],
            TPID: [0x81, 0x00],
            TCI: [0x80, 0x02],
            ehterType: [0x88, 0xB8],
            APPID: [0x00, 0x08],
            length: [0x00, 0x00],
        };
        let current_time = getTimeMs();
        let goose_data = vec![
            IECData::float32(2.0),
            IECData::float32(2.0),
            IECData::float32(4.0),
            IECData::float32(5.0),
            IECData::float32(6.0),
            IECData::float32(7.0),
            IECData::float32(8.0),
            IECData::float32(9.0),
            IECData::float32(10.0),
            IECData::float32(11.0),
            IECData::float32(12.0),
            IECData::float32(13.0),
            IECData::float32(14.0),
            IECData::float32(15.0),
            IECData::float32(16.0),
            IECData::float32(1.0),
        ];
        println!("goose data is:{:?}", goose_data);
        let mut f1 = 0.1;
        let mut f2 = 100.1;

        let mut goose_pdu = IECGoosePdu {
            gocbRef: "XD11LDevice1/LLN0$GO$Go_Gcb2".to_string(),
            timeAllowedtoLive: 6400,
            datSet: "XD11LDevice1/LLN0$dsGOOSE2".to_string(),
            goID: "XD11LDevice1/LLN0.Go_Gcb2".to_string(),
            t: current_time,
            stNum: 12,
            sqNum: 23,
            simulation: false,
            confRev: 5,
            ndsCom: false,
            numDatSetEntries: goose_data.len() as u32,
            allData: goose_data,
        };

        goose_pdu.numDatSetEntries = goose_pdu.allData.len() as u32;

        loop {
            std::thread::sleep(std::time::Duration::from_millis(2));
            f1 = f1 + 0.5;
            f2 = f2 + 0.5;

            let goose_data = vec![
                IECData::float32(f1),
                IECData::float32(f1),
                IECData::float32(f1),
                IECData::float32(f1),
                IECData::float32(f1),
                IECData::float32(f1),
                IECData::float32(f1),
                IECData::float32(f1),
                IECData::float32(f1),
                IECData::float32(f1),
                IECData::float32(f1),
                IECData::float32(f1),
                IECData::float32(f1),
                IECData::float32(f1),
                IECData::float32(f1),
                IECData::float32(f2),
            ];

            goose_pdu.stNum = goose_pdu.stNum + 1;
            goose_pdu.sqNum = 0;
            goose_pdu.numDatSetEntries = goose_pdu.allData.len() as u32;
            goose_pdu.allData = goose_data;
            let mut buffer = [0 as u8; GOOSE_BUFFER_SIZE];

            let goose_frame_size = encodeGooseFrame(&mut ether_header, &goose_pdu, &mut buffer, 0);

            tx.send_to(&buffer[..goose_frame_size], None);
        }
    });
}

