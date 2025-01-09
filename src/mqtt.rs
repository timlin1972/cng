pub struct Mqtt {}

impl Mqtt {
    pub fn new() -> Mqtt {
        Mqtt {}
    }

    pub fn connect(&mut self) {
        println!("Connecting to MQTT broker");
    }

    pub fn publish(&self, topic: &str, payload: &str) {
        println!("Publishing to topic: {} with payload: {}", topic, payload);
    }
}
