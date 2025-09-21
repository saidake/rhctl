#!/bin/bash



# Create IAM policy
aws iam create-role --role-name lambda-exec-role --assume-role-policy-document file://trust-policy.json
aws iam attach-role-policy --role-name lambda-exec-role --policy-arn arn:aws:iam::aws:policy/service-role/AWSLambdaBasicExecutionRole
#
## Create a function
aws lambda create-function \
  --function-name SimiLambdaFunction \
  --zip-file fileb://simi-lambda.zip \
  --handler org.springframework.cloud.function.adapter.aws.FunctionInvoker  \
  --runtime java17 \
  --role arn:aws:iam::123456789012:role/lambda-exec-role

