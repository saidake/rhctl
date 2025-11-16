#!/bin/bash

aws dynamodb create-table \
    --table-name TestPerson \
    --attribute-definitions AttributeName=testPersonId,AttributeType=S \
    --key-schema AttributeName=testPersonId,KeyType=HASH \
    --provisioned-throughput ReadCapacityUnits=5,WriteCapacityUnits=5

aws dynamodb create-table \
    --table-name TestStudent \
    --attribute-definitions AttributeName=testStudentId,AttributeType=S \
    --key-schema AttributeName=testStudentId,KeyType=HASH \
    --provisioned-throughput ReadCapacityUnits=5,WriteCapacityUnits=5

aws dynamodb create-table \
    --table-name TestTeacher \
    --attribute-definitions AttributeName=testTeacherId,AttributeType=S \
    --key-schema AttributeName=testTeacherId,KeyType=HASH \
    --provisioned-throughput ReadCapacityUnits=5,WriteCapacityUnits=5

# Add Initial Data to TestPerson Table
aws dynamodb put-item \
    --table-name TestPerson \
    --item '{
        "testPersonId": {"S": "001"},
        "name": {"S": "John Doe"},
        "age": {"N": "40"},
        "address": {"S": "123 Main St, New York"}
    }'

aws dynamodb put-item \
    --table-name TestPerson \
    --item '{
        "testPersonId": {"S": "002"},
        "name": {"S": "Jane Smith"},
        "age": {"N": "28"},
        "Address": {"S": "456 Oak St, Los Angeles"}
    }'
# Add Initial Data to TestStudent Table
aws dynamodb put-item \
    --table-name TestStudent \
    --item '{
        "testStudentId": {"S": "S001"},
        "name": {"S": "Emily Johnson"},
        "age": {"N": "20"},
        "address": {"S": "Computer Science"}
    }'

aws dynamodb put-item \
    --table-name TestStudent \
    --item '{
        "testStudentId": {"S": "S002"},
        "name": {"S": "Michael Brown"},
        "age": {"N": "22"},
        "address": {"S": "Mathematics"}
    }'
# Add Initial Data to TestTeacher Table
aws dynamodb put-item \
    --table-name TestTeacher \
    --item '{
        "testTeacherId": {"S": "T001"},
        "name": {"S": "Dr. Alice White"},
        "age": {"N": "45"},
        "department": {"S": "Physics"}
    }'

aws dynamodb put-item \
    --table-name TestTeacher \
    --item '{
        "testTeacherId": {"S": "T002"},
        "name": {"S": "Prof. Robert Green"},
        "age": {"N": "50"},
        "department": {"S": "Chemistry"}
    }'

# Verify Data
aws dynamodb scan --table-name TestPerson
#aws dynamodb list-tables
