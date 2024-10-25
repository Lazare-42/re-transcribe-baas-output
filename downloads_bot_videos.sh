#!/bin/bash -e

# Configuration
URL="https://api.meetingbaas.com/bots/"
PROFILE="PROD"
API_KEY="$1"
OUTPUT_DIR="./files"

# Check if the IDs file is provided
if [ "$#" -lt 2 ]; then
    echo "Usage: $0 API_KEY bot_ids.txt"
    exit 1
fi

# Create the output directory if it doesn't exist
mkdir -p "$OUTPUT_DIR"

# Read each ID from the file
while IFS= read -r BOT_ID || [[ -n "$BOT_ID" ]]; do
    echo "Processing bot ID: $BOT_ID"
    
    # Retrieve bot metadata
    RESPONSE=$(curl -s -X GET "${URL}meeting_data?bot_id=$BOT_ID" \
        -H "Content-Type: application/json" \
        -H "x-meeting-baas-api-key: $API_KEY")

    OUTPUT_FILE="$OUTPUT_DIR/${BOT_ID}.json"
    echo $RESPONSE | jq > "$OUTPUT_FILE"

    # Extract the MP4 URL with jq
    # MP4_URL=$(echo "$RESPONSE" | jq -r '.assets[0].mp4_s3_path')
    
    # if [ "$MP4_URL" != "null" ] && [ -n "$MP4_URL" ]; then
    #     echo "Downloading video for bot $BOT_ID"
    #     OUTPUT_FILE="$OUTPUT_DIR/${BOT_ID}.mp4"
    #     curl -s -L "$MP4_URL" -o "$OUTPUT_FILE"
    #     echo "Video saved in: $OUTPUT_FILE"
    # else
    #     echo "No MP4 URL found for bot $BOT_ID"
    # fi
    
    echo "----------------------------------------"
done < "$2"

echo "Download completed!"
