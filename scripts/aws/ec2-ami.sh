#!/bin/bash

#aws ec2 create-key-pair --key-name SimiKeyPair --query 'KeyMaterial' --output text > SimiKeyPair.pem
#chmod 400 SimiKeyPair.pem
#aws ec2 delete-key-pair --key-name SimiKeyPair

#aws ec2 describe-images --owners amazon --query "Images[*].[ImageId, Name]" --output table

aws ec2 run-instances \
  --image-id ami-0123456789abcdef0 \
  --instance-type t2.micro \
  --key-name SimiKeyPair \
  --security-group-ids sg-0123456789abcdef0 \
  --count 1 \
  --associate-public-ip-address

#aws ec2 describe-instances
# "PublicIpAddress": "54.214.109.170",
