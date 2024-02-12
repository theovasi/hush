use cpal::traits::HostTrait;

pub fn list_input_devices(host_id: cpal::HostId) -> Vec<cpal::Device> {
    let host = cpal::host_from_id(host_id);

    let devices = match host {
        Ok(host) => {
            let devices = match host.input_devices() {
                Ok(devices) => devices.collect(),
                Err(_) => vec![],
            };
            devices
        }
        Err(_) => {
            vec![]
        }
    };

    devices
}

pub fn get_device(index: Option<usize>, host_id: Option<cpal::HostId>) -> cpal::Device {
    match host_id {
        Some(id) => {
            let devices = list_input_devices(id);
            let device = match index {
                Some(i) => devices.into_iter().nth(i),
                None => cpal::host_from_id(id).unwrap().default_input_device(),
            };
            device.unwrap()
        }
        None => {
            let devices = list_input_devices(cpal::default_host().id());
            let device = match index {
                Some(i) => devices.into_iter().nth(i),
                None => cpal::default_host().default_input_device(),
            };
            device.unwrap()
        }
    }
}

pub fn default_input() -> Option<cpal::Device> {
    let host = cpal::default_host();
    println!("Recording on {:?}", host.id());

    match host.default_input_device() {
        Some(device) => Some(device),
        None => {
            println!(
                "Failed to get default input device for host: {:?}",
                host.id()
            );
            None
        }
    }
}
