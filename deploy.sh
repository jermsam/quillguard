#!/bin/bash

# QuillGuard Deployment Script
set -e

echo "🪶 QuillGuard Deployment"
echo "======================="

# Check if Docker is installed
if ! command -v docker &> /dev/null; then
    echo "❌ Docker is not installed. Please install Docker first."
    exit 1
fi

# Check if Docker Compose is installed
if ! command -v docker-compose &> /dev/null; then
    echo "❌ Docker Compose is not installed. Please install Docker Compose first."
    exit 1
fi

# Parse command line arguments
ENVIRONMENT=${1:-production}

case $ENVIRONMENT in
    "production"|"prod")
        echo "🚀 Deploying to PRODUCTION"
        COMPOSE_FILE="docker-compose.yml"
        ;;
    "development"|"dev")
        echo "🛠️  Deploying to DEVELOPMENT"
        COMPOSE_FILE="docker-compose.dev.yml"
        ;;
    *)
        echo "❌ Invalid environment: $ENVIRONMENT"
        echo "Usage: $0 [production|development]"
        exit 1
        ;;
esac

# Setup models if they don't exist
echo "📦 Checking models..."
if [ ! -d "flan_t5_onnx" ] || [ ! -d "gramformer_onnx" ]; then
    echo "🔧 Setting up models..."
    ./setup_models.sh
else
    echo "✅ Models already exist"
fi

# Build and start services
echo "🏗️  Building and starting services..."
docker-compose -f $COMPOSE_FILE down --remove-orphans
docker-compose -f $COMPOSE_FILE build --no-cache
docker-compose -f $COMPOSE_FILE up -d

# Wait for services to be healthy
echo "⏳ Waiting for services to be ready..."
sleep 10

# Check service health
echo "🔍 Checking service health..."

# Check backend
if curl -f http://localhost:3000/api/info &> /dev/null; then
    echo "✅ Backend is healthy"
else
    echo "❌ Backend is not responding"
    docker-compose -f $COMPOSE_FILE logs backend
    exit 1
fi

# Check frontend (different ports for prod vs dev)
if [ "$ENVIRONMENT" = "development" ] || [ "$ENVIRONMENT" = "dev" ]; then
    FRONTEND_PORT=5173
else
    FRONTEND_PORT=3001
fi

if curl -f http://localhost:$FRONTEND_PORT &> /dev/null; then
    echo "✅ Frontend is healthy"
else
    echo "❌ Frontend is not responding"
    docker-compose -f $COMPOSE_FILE logs frontend
    exit 1
fi

echo ""
echo "🎉 QuillGuard deployed successfully!"
echo ""
echo "📋 Service URLs:"
echo "   Frontend: http://localhost:$FRONTEND_PORT"
echo "   Backend API: http://localhost:3000/api"
echo "   Health Check: http://localhost:3000/api/info"
echo ""
echo "📊 Useful commands:"
echo "   View logs: docker-compose -f $COMPOSE_FILE logs -f"
echo "   Stop services: docker-compose -f $COMPOSE_FILE down"
echo "   Restart: docker-compose -f $COMPOSE_FILE restart"
echo ""
echo "🪶 Happy writing with QuillGuard! 🛡️"
