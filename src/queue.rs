//! Queue management for handling game events using AMQP

use anyhow::{anyhow, Result};
use futures_lite::stream::StreamExt;
use lapin::{
    options::*, types::FieldTable, BasicProperties, Channel, Connection, ConnectionProperties,
    ExchangeKind,
};
use std::sync::Arc;
use tracing::{error, info};

struct QueueClientInner {
    connection: Connection,
    channel: Channel,
    incoming_topic: String,
    outgoing_topic: String,
}

/// Queue client for handling game-related messages
#[derive(Clone)]
pub struct QueueClient {
    inner: Arc<QueueClientInner>,
}

impl QueueClient {
    /// Create a new queue client connected to the specified cluster URL
    pub async fn new(cluster_url: &str) -> Result<Self> {
        info!("Connecting to AMQP cluster at: {}", cluster_url);

        let connection = Connection::connect(cluster_url, ConnectionProperties::default())
            .await
            .map_err(|e| anyhow!("Failed to connect to AMQP cluster: {}", e))?;

        let channel = connection
            .create_channel()
            .await
            .map_err(|e| anyhow!("Failed to create AMQP channel: {}", e))?;

        // Declare topics/exchanges
        let incoming_topic = "game.starting".to_string();
        let outgoing_topic = "game.complete".to_string();

        // Declare exchanges for topics
        channel
            .exchange_declare(
                &incoming_topic,
                ExchangeKind::Topic,
                ExchangeDeclareOptions::default(),
                FieldTable::default(),
            )
            .await
            .map_err(|e| anyhow!("Failed to declare incoming exchange: {}", e))?;

        channel
            .exchange_declare(
                &outgoing_topic,
                ExchangeKind::Topic,
                ExchangeDeclareOptions::default(),
                FieldTable::default(),
            )
            .await
            .map_err(|e| anyhow!("Failed to declare outgoing exchange: {}", e))?;

        let inner = QueueClientInner {
            connection,
            channel,
            incoming_topic,
            outgoing_topic,
        };

        Ok(Self {
            inner: Arc::new(inner),
        })
    }

    /// Start consuming messages from the GameStarting topic
    /// The handler function will receive raw Cap'n Proto data for now
    pub async fn start_consuming<F>(&self, queue_name: &str, handler: F) -> Result<()>
    where
        F: Fn(&[u8]) -> Result<()> + Send + Sync + 'static,
    {
        info!(
            "Starting to consume messages from topic: {} on queue: {}",
            self.inner.incoming_topic, queue_name
        );

        // Declare a durable queue for consuming
        let queue = self
            .inner
            .channel
            .queue_declare(
                queue_name,
                QueueDeclareOptions {
                    durable: true,
                    ..Default::default()
                },
                FieldTable::default(),
            )
            .await
            .map_err(|e| anyhow!("Failed to declare queue: {}", e))?;

        // Bind the queue to the exchange
        self.inner
            .channel
            .queue_bind(
                &queue.name().as_str(),
                &self.inner.incoming_topic,
                "#",
                QueueBindOptions::default(),
                FieldTable::default(),
            )
            .await
            .map_err(|e| anyhow!("Failed to bind queue to exchange: {}", e))?;

        // Start consuming
        let mut consumer = self
            .inner
            .channel
            .basic_consume(
                &queue.name().as_str(),
                "game_starting_consumer",
                BasicConsumeOptions::default(),
                FieldTable::default(),
            )
            .await
            .map_err(|e| anyhow!("Failed to start consuming: {}", e))?;

        // Handle messages using the consumer directly with StreamExt
        info!("Consumer started, waiting for messages...");
        while let Some(delivery_result) = consumer.next().await {
            match delivery_result {
                Ok(delivery) => {
                    info!("Received GameStarting message");
                    if let Err(e) = handler(&delivery.data) {
                        error!("Error handling GameStarting message: {}", e);
                    }

                    // Acknowledge the message
                    if let Err(e) = delivery.ack(BasicAckOptions::default()).await {
                        error!("Failed to acknowledge message: {}", e);
                    }
                }
                Err(e) => {
                    error!("Error receiving message: {}", e);
                    return Err(e.into());
                }
            }
        }
        info!("Consumer stream finished.");

        Ok(())
    }

    /// Publish a GameStarting message to the incoming topic
    pub async fn publish_game_starting(&self, game_starting_data: &[u8]) -> Result<()> {
        info!("Publishing GameStarting message");

        let properties = BasicProperties::default()
            .with_content_type("application/capnp".into())
            .with_delivery_mode(2); // Persistent

        self.inner
            .channel
            .basic_publish(
                &self.inner.incoming_topic,
                "",
                BasicPublishOptions::default(),
                game_starting_data,
                properties,
            )
            .await
            .map_err(|e| anyhow!("Failed to publish GameStarting message: {}", e))?;

        info!("Successfully published GameStarting message");
        Ok(())
    }

    /// Publish a GameComplete message to the outgoing topic
    pub async fn publish_game_complete(
        &self,
        routing_key: &str,
        game_complete_data: &[u8],
    ) -> Result<()> {
        info!(
            "Publishing GameComplete message with routing key: {}",
            routing_key
        );

        let properties = BasicProperties::default()
            .with_content_type("application/capnp".into())
            .with_delivery_mode(2); // Persistent

        self.inner
            .channel
            .basic_publish(
                &self.inner.outgoing_topic,
                routing_key,
                BasicPublishOptions::default(),
                game_complete_data,
                properties,
            )
            .await
            .map_err(|e| anyhow!("Failed to publish GameComplete message: {}", e))?;

        info!("Successfully published GameComplete message");
        Ok(())
    }

    /// Consume one message from a topic with a specific routing key
    pub async fn consume_one(&self, topic: &str, routing_key: &str) -> Result<Vec<u8>> {
        info!(
            "Consuming one message from topic: {} with routing key: {}",
            topic, routing_key
        );

        let queue = self
            .inner
            .channel
            .queue_declare(
                "",
                QueueDeclareOptions {
                    exclusive: true,
                    auto_delete: true,
                    ..Default::default()
                },
                FieldTable::default(),
            )
            .await?;

        self.inner
            .channel
            .queue_bind(
                &queue.name().as_str(),
                topic,
                routing_key,
                QueueBindOptions::default(),
                FieldTable::default(),
            )
            .await?;

        let consumer = self
            .inner
            .channel
            .basic_consume(
                &queue.name().as_str(),
                "one_shot_consumer",
                BasicConsumeOptions::default(),
                FieldTable::default(),
            )
            .await?;

        let mut consumer_stream = consumer;
        if let Some(delivery_result) = consumer_stream.next().await {
            let delivery = delivery_result?;
            delivery.ack(BasicAckOptions::default()).await?;
            return Ok(delivery.data);
        }

        Err(anyhow!("No message received"))
    }

    pub fn outgoing_topic(&self) -> &str {
        &self.inner.outgoing_topic
    }

    /// Close the queue client connection
    pub async fn close(&self) -> Result<()> {
        info!("Closing AMQP connection");
        self.inner
            .connection
            .close(200, "Normal shutdown")
            .await
            .map_err(|e| anyhow!("Failed to close AMQP connection: {}", e))
    }
}
