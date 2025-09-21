#!/bin/bash

# Create a topic
aws sns create-topic --name SimiTopic
#   "TopicArn": "arn:aws:sns:us-east-1:000000000000:SimiTopic"

# Create a queue
aws sqs create-queue --queue-name SimiQueue
#   --queue-url http://localhost:4566/000000000000/SimiQueue

# Subscribe the queue to the topic
aws sns subscribe \
    --topic-arn "arn:aws:sns:us-east-1:000000000000:SimiTopic" \
    --protocol sqs \
    --notification-endpoint "arn:aws:sqs:us-east-1:000000000000:SimiQueue"

# List subscriptions by topic
aws sns list-subscriptions-by-topic --topic-arn "arn:aws:sns:us-east-1:000000000000:SimiTopic"
# aws sns list-topics
