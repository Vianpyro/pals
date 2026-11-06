
import java.util.Scanner;



public class Main {

    public static void main(String[] args) {

        Scanner in = new Scanner(System.in);

        long total = 0;

        while (in.hasNextLong()) total += in.nextLong();

        System.out.println(total);

    }

}

