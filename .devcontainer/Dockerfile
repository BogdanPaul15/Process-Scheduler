FROM ubuntu:22.04

# Update packages
RUN apt-get update

# Add sudo
RUN apt-get install -y sudo

# Add codespace user
RUN adduser --disabled-password --gecos '' codespace
RUN adduser codespace sudo
RUN echo '%sudo ALL=(ALL) NOPASSWD:ALL' >> /etc/sudoers
USER codespace
WORKDIR /home/codespace

# Get Ubuntu packages
RUN sudo apt-get install -y \
    build-essential \
    curl \
    git
    
# Install Rust
RUN curl https://sh.rustup.rs -sSf | sh -s -- -y 

# Add .cargo/bin to PATH
ENV PATH="/home/codespace/.cargo/bin:${PATH}"
