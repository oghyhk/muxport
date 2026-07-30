// This is a basic Flutter widget test.
//
// To perform an interaction with a widget in your test, use the WidgetTester
// utility in the flutter_test package. For example, you can send tap and scroll
// gestures. You can also use WidgetTester to find child widgets in the widget
// tree, read text, and verify that the values of widget properties are correct.

import 'package:flutter_test/flutter_test.dart';

import 'package:muxport_mobile/main.dart';

void main() {
  testWidgets('opens the host fleet and switches to sessions', (tester) async {
    await tester.pumpWidget(const MuxportApp());

    expect(find.text('Host Fleet'), findsOneWidget);
    expect(find.text('Hosts'), findsOneWidget);

    await tester.tap(find.text('Sessions'));
    await tester.pumpAndSettle();

    expect(find.text('Unified Session Timeline'), findsOneWidget);
  });
}
