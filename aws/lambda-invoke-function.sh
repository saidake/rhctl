#!/bin/bash

# public Function<String, String> greet()
# public Function<Integer, String> square()

# Execute function with aws cli
payload='{"functionName": "greet", "data": "Alice"}'
encoded_payload=$(echo -n $payload | base64)
aws lambda invoke \
    --function-name SimiLambdaFunction \
    --payload "$encoded_payload" \
    output.json
# Update function with aws cli
#
#aws lambda update-function-code \
#  --function-name SimiLambdaFunction \
#  --zip-file fileb://simi-webflux-1.0-SNAPSHOT.jar

echo 'print output.json: ' && cat output.json
