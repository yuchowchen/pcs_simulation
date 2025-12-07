use anyhow::Result;
use log::{error, info, warn};
use pnet_datalink::{self, Channel, DataLinkReceiver, DataLinkSender};

/// Network channel setup result containing transmitters and receivers for both LANs
pub struct NetworkChannels {
    pub tx_lan1: Option<Box<dyn DataLinkSender>>,
    pub rx_lan1: Option<Box<dyn DataLinkReceiver>>,
    pub tx_lan2: Option<Box<dyn DataLinkSender>>,
    pub rx_lan2: Option<Box<dyn DataLinkReceiver>>,
}

/// Creates datalink channels for both LAN1 and LAN2 interfaces
/// 
/// # Arguments
/// * `interfaces` - Vector of network interfaces (expects at least 2 interfaces)
/// 
/// # Returns
/// * `Result<NetworkChannels>` - Network channels or error
pub fn setup_network_channels(
    interfaces: &[pnet_datalink::NetworkInterface],
) -> Result<NetworkChannels> {
    if interfaces.len() < 2 {
        anyhow::bail!(
            "Insufficient network interfaces: expected 2, found {}",
            interfaces.len()
        );
    }

    // Create channel for LAN1
    let (tx_lan1_opt, rx_lan1_opt) = match pnet_datalink::channel(&interfaces[0], Default::default())
    {
        Ok(Channel::Ethernet(tx, rx)) => {
            info!(
                "Successfully created LAN1 channel on interface: {}",
                interfaces[0].name
            );
            (Some(tx), Some(rx))
        }
        Ok(_) => {
            error!("Unhandled channel type for LAN1");
            (None, None)
        }
        Err(e) => {
            error!("Failed to create LAN1 datalink channel: {}", e);
            (None, None)
        }
    };

    // Create channel for LAN2
    let (tx_lan2_opt, rx_lan2_opt) = match pnet_datalink::channel(&interfaces[1], Default::default())
    {
        Ok(Channel::Ethernet(tx, rx)) => {
            info!(
                "Successfully created LAN2 channel on interface: {}",
                interfaces[1].name
            );
            (Some(tx), Some(rx))
        }
        Ok(_) => {
            error!("Unhandled channel type for LAN2");
            (None, None)
        }
        Err(e) => {
            error!("Failed to create LAN2 datalink channel: {}", e);
            (None, None)
        }
    };

    // Check if both channels failed
    if tx_lan1_opt.is_none() && tx_lan2_opt.is_none() {
        anyhow::bail!("Both LAN1 and LAN2 channel creation failed - cannot continue");
    }

    // Log operational status
    match (&rx_lan1_opt, &rx_lan2_opt) {
        (Some(_), Some(_)) => info!("Both LAN1 and LAN2 channels operational - full redundancy"),
        (Some(_), None) => warn!("Only LAN1 channel operational - running in degraded mode"),
        (None, Some(_)) => warn!("Only LAN2 channel operational - running in degraded mode"),
        (None, None) => {
            error!("Both channels failed but should have returned error above");
        }
    }

    Ok(NetworkChannels {
        tx_lan1: tx_lan1_opt,
        rx_lan1: rx_lan1_opt,
        tx_lan2: tx_lan2_opt,
        rx_lan2: rx_lan2_opt,
    })
}
