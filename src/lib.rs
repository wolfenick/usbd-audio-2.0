#![no_std]
#[allow(unused_parens)]

/// INCLUDES
use usb_device::{
    UsbError,
    class_prelude::*,
    endpoint::{
        Endpoint,
        EndpointDirection,
        In,
        Out,
    },
    device::{
        DEFAULT_ALTERNATE_SETTING,
    },
    control::{
        Recipient,
        Request,
        RequestType
    }
};

use core::marker::PhantomData;

// LOCAL INCLUDES
mod class_codes;
mod terminal_type;

use class_codes::*;
pub use terminal_type::TerminalType;
use usb_device::{
    descriptor::descriptor_type::INTERFACE,
    endpoint::{
        IsochronousSynchronizationType::Asynchronous,
        IsochronousUsageType::{Data, ImplicitFeedbackData},
    },
};

// CONSTANTS
const ID_CLOCK_SRC: u8 = 0x01;

const ID_INPUT_TERMINAL: u8 = 0x02;
const ID_INPUT_STREAMING: u8 = 0x03;

const ID_OUTPUT_TERMINAL: u8 = 0x05;
const ID_OUTPUT_STREAMING: u8 = 0x04;



// ERROR DEFINITIONS
#[derive(Debug)]
pub enum Error{
    UsbError(UsbError),
    StreamNotInitialized
}
type Result<T> = core::result::Result<T, Error>;



/// STREAM CONFIG
#[derive(Clone, Copy, Debug)]
pub enum Format {
    S16LE,
    S24LE,
}

impl Format {

    fn size(&self) -> u8 {
        match self {
            Format::S16LE => 2,
            Format::S24LE => 3,
        }
    }

    fn res(&self) -> u8 {
        match self {
            Format::S16LE => 16,
            Format::S24LE => 24,
        }
    }

}

pub struct StreamConfig<'a> {
    format: Format,
    rate: u16,
    term_type: TerminalType,
    n_channels: u8,
    marker: PhantomData<&'a u8>,
}

impl<'a> StreamConfig<'a> {

    pub fn new(format: Format, rate: u16, n_channels: u8, term_type: TerminalType) -> Result<StreamConfig<'a>>{
        Ok(
            StreamConfig {
                format,
                rate,
                n_channels,
                term_type,
                marker: PhantomData
            }
        )
    }

    pub fn packet_size(&self) -> u16 {
        // number of bytes for one sample
        let size = self.format.size() * self.n_channels;

        // this integer division causes a necessary floor round
        let samples = (self.rate / 1000);

        // we need to satisfy n + 1 audio samples as the maximum for feedback compensation
        (samples + 1) * size
    }

}



/// AUDIO STREAM
pub struct AudioStream<'a, B: UsbBus, D: EndpointDirection> {
    stream_config: StreamConfig<'a>,
    interface: InterfaceNumber,
    endpoint: Endpoint<'a, B, D>,
    alt_setting: u8,
}

impl<'a, B: UsbBus, D: EndpointDirection> AudioStream<'a, B, D> {

    fn input_ac_descriptor(&self, writer: &mut DescriptorWriter) -> usb_device::Result<()> {

        let input_type: [u8; 2] = self.stream_config.term_type.as_bytes();
        let output_type: [u8; 2] = TerminalType::UsbStreaming.as_bytes();

        writer.write(CS_INTERFACE, &[
            INPUT_TERMINAL,
            ID_INPUT_TERMINAL, // terminal ID
            input_type[0], // terminal type
            input_type[1],
            0x00, // associated terminal (no assoc)
            ID_CLOCK_SRC, // clock source ID
            self.stream_config.n_channels, // logical channels
            0x00, 0x00, 0x00, 0x00, // spacial description config
            0x00, // string index (none)
            0x00, 0x00, // bmControls (none)
            0x00, // terminal desc string index (none)
        ]).unwrap();

        writer.write(CS_INTERFACE, &[
            OUTPUT_TERMINAL,
            ID_INPUT_STREAMING, // terminal ID
            output_type[0], // terminal type
            output_type[1],
            0x00, // associated terminal (none)
            ID_INPUT_TERMINAL, // source ID (the above input terminal)
            ID_CLOCK_SRC, // clock source ID (none)
            0x00, // bmControls (none)
            0x00,
            0x00, // terminal desc string index (none)
        ]).unwrap();

        Ok(())

    }

    fn output_ac_descriptor(&self, writer: &mut DescriptorWriter) -> usb_device::Result<()> {

        let input_type: [u8; 2] = TerminalType::UsbStreaming.as_bytes();
        let output_type: [u8; 2] = self.stream_config.term_type.as_bytes();

        writer.write(CS_INTERFACE, &[
            INPUT_TERMINAL,
            ID_OUTPUT_STREAMING, // terminal ID
            input_type[0], // terminal type
            input_type[1],
            0x00, // associated terminal (no assoc)
            ID_CLOCK_SRC, // clock source ID
            self.stream_config.n_channels, // logical channels
            0x00, 0x00, 0x00, 0x00, // spacial description config
            0x00, // string index (none)
            0x00, 0x00, //bmControls (none)
            0x00, // terminal desc string index (none)
        ]).unwrap();

        writer.write(CS_INTERFACE, &[
            OUTPUT_TERMINAL,
            ID_OUTPUT_TERMINAL, // terminal ID
            output_type[0], // terminal type
            output_type[1],
            0x00, // associated terminal (none)
            ID_OUTPUT_STREAMING, //source ID (the above input terminal)
            ID_CLOCK_SRC, // clock source ID (none)
            0x00, // bmControls (none)
            0x00,
            0x00, // terminal desc string index (none)
        ]).unwrap();

        Ok(())
    }

    fn input_as_ep_descriptor(&self, writer: &mut DescriptorWriter) -> usb_device::Result<()> {

        // AUDIO STREAMING DESCRIPTORS
        //TODO check the protocol value (IP_VERSION_02_00)
        writer.interface(self.interface, AUDIO, AUDIOSTREAMING, IP_VERSION_02_00).unwrap();

        writer.write(INTERFACE, &[
            self.interface.into(),
            0x01, // alternate setting
            0x01, // n endpoints (1 data endpoint)
            AUDIO,
            AUDIOSTREAMING,
            IP_VERSION_02_00,
            0x00,
        ]).unwrap();

        writer.write(CS_INTERFACE, &[
            AS_GENERAL,
            ID_INPUT_STREAMING, // input interface ID (USB streaming)
            0x00, // bmControls
            0x01, // format type I
            0x01, 0x00, 0x00, 0x00, // audio data formats (PCM only)
            self.stream_config.n_channels,
            0x00, 0x00, 0x00, 0x00, // spacial location description (none)
            0x00, // string index (none)
        ]).unwrap();

        writer.write(CS_INTERFACE, &[
            FORMAT_TYPE,
            FORMAT_TYPE_I,
            self.stream_config.format.size(),
            self.stream_config.format.res(),
        ]).unwrap();

        // ENDPOINT DESCRIPTORS
        /*
        The standard writer endpoint function doesn't allow for the custom bmAttributes
        necessary for implicit feedback, or to define the synchronisation type. So,
        this is done manually with the fields filled from the endpoint where needed.
         */
        let max_transfer: [u8; 2] = self.stream_config.packet_size().to_be_bytes();

        writer.write(0x05, &[
            self.endpoint.address().into(),
            0b00100101, // bmAttributes: Isochronous, Implicit FB, Asynchronous
            max_transfer[1],
            max_transfer[0],
            self.endpoint.interval(),
        ]).unwrap();

        writer.write(CS_ENDPOINT, &[
            EP_GENERAL,
            0x00, // bmAttributes
            0x00, // bmControls
            0x00, // bLockDelayUnits
            0x00, 0x00 // wLockDelay
        ]).unwrap();

        Ok(())

    }

    fn output_as_ep_descriptor(&self, writer: &mut DescriptorWriter) -> usb_device::Result<()> {

        // AUDIO STREAMING DESCRIPTORS
        writer.interface(self.interface, AUDIO, AUDIOSTREAMING, IP_UNDEFINED).unwrap();

        writer.write(INTERFACE, &[
            self.interface.into(),
            0x01, // alternate setting
            0x01, // n endpoints (1 data endpoint)
            AUDIO,
            AUDIOSTREAMING,
            IP_VERSION_02_00,
            0x00,
        ]).unwrap();

        writer.write(CS_INTERFACE, &[
            AS_GENERAL,
            ID_OUTPUT_STREAMING,
            0x00,
            0x01,
            0x01, 0x00, 0x00, 0x00,
            self.stream_config.n_channels,
            0x00, 0x00, 0x00, 0x00,
            0x00,
        ]).unwrap();

        writer.write(CS_INTERFACE, &[
            FORMAT_TYPE,
            FORMAT_TYPE_I,
            self.stream_config.format.size(),
            self.stream_config.format.res(),
        ]).unwrap();

        let max_transfer: [u8; 2] = self.stream_config.packet_size().to_be_bytes();

        writer.write(0x05, &[
            self.endpoint.address().into(),
            0b00000101, // bmAttributes: Isochronous, Asynchronous
            max_transfer[1],
            max_transfer[0],
            self.endpoint.interval(),
        ]).unwrap();

        writer.write(CS_ENDPOINT, &[
            EP_GENERAL,
            0x00, // bmAttributes
            0x00, // bmControls
            0x00, // bLockDelayUnits
            0x00, 0x00 // wLockDelay
        ]).unwrap();

        Ok(())

    }

}



/// AUDIO CLASS
pub struct AudioClass<'a, B: UsbBus> {
    control_interface: InterfaceNumber,
    input: Option<AudioStream<'a, B, In>>,
    output: Option<AudioStream<'a, B, Out>>,
    clock_index: u8,
}

impl<B: UsbBus> AudioClass<'_, B> {

    /// Read audio frames as output by the host. Returns an Error if no output
    /// stream has been configured.
    pub fn read(&self, data: &mut [u8]) -> Result<usize> {

        if let Some(ref output) = self.output {
            output.endpoint.read(data).map_err(Error::UsbError)
        } else {
            Err(Error::StreamNotInitialized)
        }

    }

    /// Write audio frames to be input by the host. Returns an Error when no
    /// input stream has been configured.
    pub fn write(&self, data: &[u8]) -> Result<usize> {
        if let Some(ref input) = self.input {
            input.endpoint.write(data).map_err(Error::UsbError)
        } else {
            Err(Error::StreamNotInitialized)
        }
    }

    /// Get current Alternate Setting of the input stream. Returns an error if
    /// the stream is not configured.
    pub fn input_alt_setting(&self) -> Result<u8> {
        self.input
            .as_ref()
            .ok_or(Error::StreamNotInitialized)
            .map(|si| si.alt_setting)
    }

    /// Get current Alternate Setting of the output stream. Returns an error if
    /// the stream is not configured.
    pub fn output_alt_setting(&self) -> Result<u8> {
        self.output
            .as_ref()
            .ok_or(Error::StreamNotInitialized)
            .map(|si| si.alt_setting)
    }

}

impl<B: UsbBus> UsbClass<B> for AudioClass<'_, B> {

    fn get_configuration_descriptors(&self, writer: &mut DescriptorWriter) -> usb_device::Result<()> {

        // PREAMBLE CALCULATIONS
        let n_interfaces: u8 =
            if let Some(ref input) = self.input { 1 } else { 0 }
            + if let Some(ref output) = self.output { 1 } else { 0 };

        let total_length: [u8; 2] =
            ((9 + 8 + (29 * n_interfaces)) as u16).to_be_bytes();

        // INTERFACE ASSOCIATION DESCRIPTOR
        writer.write(0x0B, &[
            0x00, // first interface
            n_interfaces + 1, // number of interfaces
            AUDIO_FUNCTION,
            FUNCTION_SUBCLASS_UNDEFINED,
            AF_VERSION_02_00,
            0x00,
        ]).unwrap();

        // BASE INTERFACE DESCRIPTOR
        writer.interface(self.control_interface, AUDIO, AUDIOCONTROL, IP_VERSION_02_00).unwrap();

        // AUDIO CONTROL HEADER
        let ac_header: [u8; 7] = [
            HEADER,
            0x00, // bcdADC 2.00 as big-endian BCD
            0x02,
            0x00, // bCategory (none)
            total_length[1],
            total_length[0],
            0x00, // bmControls (none)
        ];

        writer.write(CS_INTERFACE, &ac_header);

        // CLOCK SOURCE DESCRIPTOR
        writer.write(CS_INTERFACE, &[
            0x0A, // CLOCK_SOURCE subtype
            ID_CLOCK_SRC,
            0b00000001, // internal fixed clock
            0b00000001, // bmControls: clock frequency read only
            0x00, // assoc terminal (none)
            0x00, // string index (none)
        ]).unwrap();

        // AUDIO CONTROL INTERFACE DESCRIPTORS
        if let Some(ref input) = self.input {
            input.input_ac_descriptor(writer).unwrap();
        }

        if let Some(ref output) = self.output {
            output.output_ac_descriptor(writer).unwrap();
        }

        // TERMINAL ENDPOINT DESCRIPTORS
        if let Some(ref input) = self.input {
            input.input_as_ep_descriptor(writer).unwrap();
        }

        if let Some(ref output) = self.output {
            output.output_as_ep_descriptor(writer).unwrap();
        }

        Ok(())

    }

    fn control_in(&mut self, xfer: ControlIn<B>) {

        let req = xfer.request();

        if (
            req.request_type == RequestType::Standard
            && req.recipient == Recipient::Interface
            && req.request == Request::GET_INTERFACE
            && req.length == 1
        ) {
            let interface = req.index as u8;

            if let Some(input) = self.input.as_ref() {
                if interface == input.interface.into() {
                    xfer.accept_with(&[input.alt_setting]).ok();
                    return;
                }
            }

            if let Some(output) = self.output.as_ref() {
                if interface == output.interface.into() {
                    xfer.accept_with(&[output.alt_setting]).ok();
                    return;
                }
            }
        }

        else if (
            req.request_type == RequestType::Class
                && req.recipient == Recipient::Interface
                && ((req.index as u16) >> 8) as u8 == ID_CLOCK_SRC
                && ((req.value as u16) >> 8) == 0x01 // clock freq control selector
        ) {

            // range request
            if (req.request == 0x02) {
                match self.clock_index {
                    0 => {
                        xfer.accept_with(&[
                            0x01, 0x00
                        ]).ok();
                        self.clock_index = 1;
                        return;
                    }
                    _ => {
                        xfer.accept_with(&[
                            0x01, 0x00, // subranges
                            0x80, 0x3E, 0x00, 0x00, // min
                            0x80, 0x3E, 0x00, 0x00, // max
                            0x01, 0x00, 0x00, 0x00  // res
                        ]).ok();
                        self.clock_index += 1;
                        return;
                    }
                }
            }

            // current value request
            else if (req.request == 0x01) {
                xfer.accept_with(&[
                    0x80, 0x3E, 0x00, 0x00
                ]).ok();
                return;
            }

        }
    }

    fn control_out(&mut self, xfer: ControlOut<B>) {

        let req = xfer.request();

        if (
            req.request_type == RequestType::Standard
            && req.recipient == Recipient::Interface
            && req.request == Request::SET_INTERFACE
        ) {

            let interface = req.index as u8;
            let alt_setting = req.value as u8;

            if let Some(input) = self.input.as_mut() {
                if interface == input.interface.into() {
                    input.alt_setting = alt_setting;
                    xfer.accept().ok();
                    return;
                }
            }

            if let Some(output) = self.output.as_mut() {
                if interface == output.interface.into() {
                    output.alt_setting = alt_setting;
                    xfer.accept().ok();
                    return;
                }
            }

        }

    }
    
}



/// AUDIO CLASS BUILDER
pub struct AudioClassBuilder<'a> {
    input: Option<StreamConfig<'a>>,
    output: Option<StreamConfig<'a>>,
    marker: PhantomData<&'a u8>,
}

impl<'a> AudioClassBuilder<'a> {

    pub fn new() -> AudioClassBuilder<'static> {
        AudioClassBuilder {
            input: None,
            output: None,
            marker: PhantomData,
        }
    }

    pub fn input(self, input: StreamConfig<'a>) -> AudioClassBuilder<'a> {
        AudioClassBuilder {
            input: Some(input),
            output: self.output,
            marker: self.marker,
        }
    }

    pub fn output(self, output: StreamConfig<'a>) -> AudioClassBuilder<'a> {
        AudioClassBuilder {
            input: self.input,
            output: Some(output),
            marker: self.marker,
        }
    }

    pub fn build<B: UsbBus>(self, allocator: &'a UsbBusAllocator<B>) -> Result<AudioClass<'a, B>> {

        let mut ac = AudioClass {
            control_interface: allocator.interface(),
            input: None,
            output: None,
            clock_index: 0,
        };

        if let Some(input_config) = self.input {

            let input_interface = allocator.interface();

            let input_endpoint = allocator.alloc(
                None,
                EndpointType::Isochronous {
                    synchronization: Asynchronous,
                    usage: ImplicitFeedbackData,
                },
                input_config.packet_size(),
                1
            ).unwrap();

            ac.input = Some(
                AudioStream {
                    stream_config: input_config,
                    interface: input_interface,
                    endpoint: input_endpoint,
                    alt_setting: DEFAULT_ALTERNATE_SETTING,
                }
            )
        }

        if let Some(output_config) = self.output {

            let output_interface = allocator.interface();

            let output_endpoint = allocator.alloc(
                None,
                EndpointType::Isochronous {
                    synchronization: Asynchronous,
                    usage: Data,
                },
                output_config.packet_size(),
                1
            ).unwrap();

            ac.output = Some(
                AudioStream {
                    stream_config: output_config,
                    interface: output_interface,
                    endpoint: output_endpoint,
                    alt_setting: DEFAULT_ALTERNATE_SETTING,
                }
            )
        }

        Ok(ac)
    }

}