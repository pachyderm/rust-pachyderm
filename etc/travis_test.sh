#!/bin/bash
cargo run --example hello_world -- $(minikube ip):30650
cargo run --example opencv -- $(minikube ip):30650
