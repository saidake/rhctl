#!/bin/sh

# ************************************************************************************
# This bash file can run in your local environment to deploy jar file to the remote server
# and control the execution of the jar file like restart, stop etc.
#
# Prerequisites:
#   Modify the required configurations in the bash file with your own values.
#
# Usage
#   ./simidep.sh deploy    - Deploy the jar file
#   ./simidep.sh start     - Start the application
#   ./simidep.sh restart   - Restart the application
#   ./simidep.sh stop      - Stop the application
#   ./simidep.sh log       - Print log file
#   ./simidep.sh info      - Show application info
#   ./simidep.sh status    - Display application status
#   ./simidep.sh debug     - Start application in debug mode
#   ./simidep.sh startjmx  - Start application with JMX enabled
#
# Author: Craig Brown
# Since: 1.1.0
# Date: Oct 6, 2024
# ************************************************************************************

#========================================================== Required Configurations
# The root directory where the application is deployed
APP_HOME=/root
# The port the application will listen on
APP_PORT=8080
# The active Spring profile (e.g. default, dev, prod).
APP_ACTIVE=default
# The name of the JAR file containing the application (This file must exist in the APP_HOME directory).
APP_JAR_NAME=simi-webflux-1.0-SNAPSHOT.jar
# The path where the application logs will be stored
JAR_LOG_PATH=/tmp/simi.log

#========================================================== Optional Configurations
# Declare a global variable to determine whether to use SSH or execute locally
USE_SSH=true  # Set to true for SSH execution, false for local execution

# If the current bash file is executed via ssh, the remote server user and host need to be configured.
REMOTE_USER=root
REMOTE_HOST='192.168.127.128'
REMOTE_APP_HOME=/root/
REMOTE_APP_JAR_NAME=simi-webflux-1.0-SNAPSHOT.jar

#JAVA_OPTS="-Xms512m -Xmx512m -Xmn256m -Djava.awt.headless=true -XX:MaxPermSize=128m -Duser.timezone=GMT+8"
JAVA_OPTS="-Xms512m -Xmx512m -Xmn256m -Djava.awt.headless=true -Duser.timezone=GMT+8"
JAVA_OPTS_JMX="$JAVA_OPTS -Dcom.sun.management.jmxremote=true -Dcom.sun.management.jmxremote.port=1099 -Dcom.sun.management.jmxremote.authenticate=false -Dcom.sun.management.jmxremote.ssl=false"
DEBUG_OPTS="-Xdebug -Xrunjdwp:transport=dt_socket,server=y,suspend=n,address="

#========================================================== Functions

# Function to execute commands via SSH
exec_command() {
    local ssh_command="$1"
    if [ "$USE_SSH" = true ]; then
        # Execute the command remotely via SSH
        ssh $REMOTE_USER@$REMOTE_HOST "bash -l -c '${ssh_command}'"
        #ssh $REMOTE_USER@$REMOTE_HOST "bash -l -c 'echo \$JAVA_HOME'"
    else
        # Execute the command locally
        eval "$ssh_command"
    fi
}

exec_command "cd $APP_HOME"

# Display application information
info() {
    echo "*****************************************************"
    echo "App Home:     $APP_HOME"
    echo "App Port:     $APP_PORT"
    echo "App Profile:  $APP_ACTIVE"
    echo "App JAR:      $APP_JAR_NAME"
    echo "Log Path:     $JAR_LOG_PATH"

    java_home=$(exec_command "echo \$JAVA_HOME")
    cur_dir=$(exec_command "pwd")
    echo "JAVA_HOME:     $java_home"
    echo "Current Dir:   $cur_dir"
    echo "*****************************************************"
}

# Function to check if the application is running
checkpid() {
    psid=$(ssh $REMOTE_USER@$REMOTE_HOST "pgrep -f $APP_JAR_NAME")
    psid=${psid:-0}
}

# Start the application
start() {
    echo "*****************************************************"
    info
    checkpid
    if [ $psid -ne 0 ]; then
        echo "Program $APP_JAR_NAME is already running! (PID=$psid)"
    else
        echo "Starting $APP_JAR_NAME..."
        exec_command "nohup java $JAVA_OPTS -jar $APP_HOME/$APP_JAR_NAME --spring.profiles.active=$APP_ACTIVE --server.port=$APP_PORT >> $JAR_LOG_PATH 2>&1 &"
        checkpid
        if [ $psid -ne 0 ]; then
            echo "$APP_JAR_NAME started successfully! (PID=$psid)"
        else
            echo "$APP_JAR_NAME failed to start!"
        fi
    fi
    echo "*****************************************************"
}

# Start the application with JMX enabled
startjmx() {
    echo "*****************************************************"
    checkpid
    if [ $psid -ne 0 ]; then
        echo "Program $APP_JAR_NAME is already running! (PID=$psid)"
    else
        echo "Starting $APP_JAR_NAME with JMX enabled..."
        exec_command "nohup java $JAVA_OPTS_JMX -jar $APP_HOME/$APP_JAR_NAME >> $JAR_LOG_PATH 2>&1 &"
        checkpid
        if [ $psid -ne 0 ]; then
            echo "$APP_JAR_NAME started with JMX! (PID=$psid)"
        else
            echo "$APP_JAR_NAME failed to start with JMX!"
        fi
    fi
    echo "*****************************************************"
}

# Start the application in debug mode
debug() {
    local debug_port=$2
    if [ -z "$debug_port" ]; then
        echo "Debug port not provided."
        return
    fi
    echo "*****************************************************"
    checkpid
    if [ $psid -ne 0 ]; then
        echo "Program $APP_JAR_NAME is already running! (PID=$psid)"
    else
        echo "Starting $APP_JAR_NAME in debug mode (Port: $debug_port)..."
        exec_command "nohup java $DEBUG_OPTS$debug_port -jar $APP_HOME/$APP_JAR_NAME >> $JAR_LOG_PATH 2>&1 &"
        checkpid
        if [ $psid -ne 0 ]; then
            echo "$APP_JAR_NAME started in debug mode! (PID=$psid, Debug port=$debug_port)"
        else
            echo "$APP_JAR_NAME failed to start in debug mode!"
        fi
    fi
    echo "*****************************************************"
}

# Stop the application
stop() {
    echo "*****************************************************"
    checkpid
    if [ $psid -ne 0 ]; then
        echo "Stopping $APP_JAR_NAME (PID=$psid)..."
        exec_command "kill -2 $psid"
        if [ $? -eq 0 ]; then
            echo "$APP_JAR_NAME stopped successfully!"
        else
            echo "Failed to stop $APP_JAR_NAME."
        fi
    else
        echo "$APP_JAR_NAME is not running."
    fi
    echo "*****************************************************"
}

# Restart the application
restart() {
    stop
    sleep 1
    start
}

# Display the current status of the application
status() {
    checkpid
    if [ $psid -ne 0 ]; then
        echo "$APP_JAR_NAME is running (PID=$psid)."
    else
        echo "$APP_JAR_NAME is not running."
    fi
}

# Tail the log file
log() {
    exec_command "tail -f $JAR_LOG_PATH"
}

#========================================================== Script Execution
# Function to display usage information
usage() {
    echo "Usage: $0 {start|stop|restart|status|log|info|startjmx|debug <debug_port>}"
    echo "Parameters:"
    echo "  start         - Start the application"
    echo "  stop          - Stop the application"
    echo "  restart       - Restart the application"
    echo "  status        - Display application status"
    echo "  log           - Print log file"
    echo "  info          - Show application info"
    echo "  startjmx      - Start application with JMX enabled"
    echo "  debug <port>  - Start application in debug mode with specified port"
    exit 1
}

case $1 in
    'start') start ;;
    'stop') stop ;;
    'restart') restart ;;
    'status') status ;;
    'log') log ;;
    'info') info ;;
    'startjmx') startjmx ;;
    'debug') debug $2 ;;
    *)
        echo "Error: Invalid command '$1'."
        usage  # Call the usage function to display correct usage information
        ;;
esac
