#!/bin/bash
set -ex
cargo run --example hello_world -- grpc://$(minikube ip):30650
cargo run --example opencv -- grpc://$(minikube ip):30650
