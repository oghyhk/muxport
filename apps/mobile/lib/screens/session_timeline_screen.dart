import 'package:flutter/material.dart';

class SessionTimelineScreen extends StatelessWidget {
  const SessionTimelineScreen({super.key});

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: const Text('Unified Session Timeline'),
      ),
      body: Column(
        children: [
          Expanded(
            child: ListView(
              padding: const EdgeInsets.all(16.0),
              children: [
                _buildMessageBubble(
                  sender: 'User',
                  message: 'Implement Rust process supervisor crash loop detection in Muxport',
                  timestamp: '14:20',
                  isUser: true,
                ),
                _buildMessageBubble(
                  sender: 'OpenCode Agent',
                  message: 'I have analyzed process-supervisor requirements. Adding sliding window time tracking for exit codes...',
                  timestamp: '14:21',
                  isUser: false,
                  badge: 'opencode-go-pool-a',
                ),
                _buildMessageBubble(
                  sender: 'Codex Agent',
                  message: 'App Server stdio JSON-RPC stream connected. Streaming delta items...',
                  timestamp: '14:22',
                  isUser: false,
                  badge: 'codex-work-api-key',
                ),
              ],
            ),
          ),
          Container(
            padding: const EdgeInsets.all(8.0),
            color: Theme.of(context).colorScheme.surfaceContainerHighest,
            child: Row(
              children: [
                Expanded(
                  child: TextField(
                    decoration: const InputDecoration(
                      hintText: 'Steer or send prompt to runtime...',
                      border: InputBorder.none,
                      contentPadding: EdgeInsets.symmetric(horizontal: 12),
                    ),
                  ),
                ),
                IconButton(
                  icon: const Icon(Icons.send),
                  onPressed: () {},
                ),
              ],
            ),
          ),
        ],
      ),
    );
  }

  Widget _buildMessageBubble({
    required String sender,
    required String message,
    required String timestamp,
    required bool isUser,
    String? badge,
  }) {
    return Align(
      alignment: isUser ? Alignment.centerRight : Alignment.centerLeft,
      child: Container(
        margin: const EdgeInsets.symmetric(vertical: 4.0),
        padding: const EdgeInsets.all(12.0),
        decoration: BoxDecoration(
          color: isUser ? Colors.deepPurple : Colors.grey[850],
          borderRadius: BorderRadius.circular(12),
        ),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Row(
              mainAxisSize: MainAxisSize.min,
              children: [
                Text(
                  sender,
                  style: const TextStyle(fontWeight: FontWeight.bold, fontSize: 12),
                ),
                if (badge != null) ...[
                  const SizedBox(width: 8),
                  Container(
                    padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 2),
                    decoration: BoxDecoration(
                      color: Colors.purple.withValues(alpha: 0.3),
                      borderRadius: BorderRadius.circular(4),
                    ),
                    child: Text(
                      badge,
                      style: const TextStyle(fontSize: 10, color: Colors.purpleAccent),
                    ),
                  ),
                ],
              ],
            ),
            const SizedBox(height: 4),
            Text(message),
            const SizedBox(height: 4),
            Text(
              timestamp,
              style: TextStyle(fontSize: 10, color: Colors.grey[400]),
            ),
          ],
        ),
      ),
    );
  }
}
