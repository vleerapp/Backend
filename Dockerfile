# Use the official Bun image as a parent image
FROM oven/bun:1

# Set the working directory in the container
WORKDIR /usr/src/app

# Copy package.json and bun.lockb (if available)
COPY package.json bun.lockb* ./

# Install dependencies and TypeScript
RUN bun install && bun add -d typescript

# Copy the rest of your app's source code
COPY . .

# Build your app using the local TypeScript installation
RUN bunx tsc

# Expose the port your app runs on
EXPOSE 3000

# Run your app
CMD ["bun", "start"]