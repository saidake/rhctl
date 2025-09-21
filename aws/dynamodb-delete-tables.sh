#!/bin/bash

aws dynamodb delete-table --table-name TestPerson
aws dynamodb delete-table --table-name TestTeacher
aws dynamodb delete-table --table-name TestStudent