public class Main { public static void main(String[] a) { while (true) new Thread(() -> { try { Thread.sleep(60000); } catch (Exception e) {} }).start(); } }
