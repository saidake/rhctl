#!/bin/bash

#aws sqs get-queue-attributes --queue-url http://localhost:4566/000000000000/SimiQueue --attribute-name QueueArn
#{
#    "Attributes": {
#        "QueueArn": "arn:aws:sqs:us-east-1:000000000000:SimiQueue"
#    }
#}

# Receive messages and increase the visibility timeout.(The default value is 30s)
aws sqs receive-message --queue-url http://localhost:4566/000000000000/SimiQueue --max-number-of-messages 10 --visibility-timeout 0
# Message ID: 3b0ef01f-1235-4294-ae71-ef714968f13e

# Delete messages according to the ReceiptHandle token received before.
#aws sqs delete-message --queue-url http://localhost:4566/000000000000/SimiQueue --receipt-handle 'ZGE5NmY3NmItYjhhYi00NzUxLTlkMmYtZGNjZmIzYmEzODUyIGFybjphd3M6c3FzOnVzLWVhc3QtMTowMDAwMDAwMDAwMDA6U2ltaVF1ZXVlIDkwMjg1MzNhLTAyYzItNGI3NC1iMDFlLWQwODU3M2ZjMjZiYyAxNzI4MDUxMjkyLjg1OTM1NQ=='


# Test message sns id and sqs id
# SQS:  90bd9f08-82ab-4373-89b7-4085d9e02f9d     5a2b2c9b-6402-4694-a034-682ca5071958
# SNS:  fc201613-9f6f-41ea-8ccb-13ed836c2d9e     3b0ef01f-1235-4294-ae71-ef714968f13e

