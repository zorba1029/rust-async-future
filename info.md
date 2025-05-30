Medium source article:

Part 1:
https://medium.com/@alfred.weirich/developing-asynchronous-components-in-rust-bea1db85c720

Part 2:
https://medium.com/@alfred.weirich/developing-asynchronous-components-in-rust-part-2-async-in-poll-79edb37188af

Part 3:
https://medium.com/@alfred.weirich/developing-asynchronous-components-in-rust-part-3-implementing-futures-ab799efffb86

Part 4:
https://medium.com/@alfred.weirich/developing-asynchronous-components-in-rust-part-4-waking-57760d3b630b

Part 5:
https://medium.com/@alfred.weirich/developing-asynchronous-components-in-rust-part-5-tokio-streams-d9ae2973f16e

Part 6: (END)
https://medium.com/@alfred.weirich/developing-asynchronous-components-in-rust-part-6-tokio-streams-42366e470e7b


Rust의 비동기 생태계(Future, async/await, poll, Waker, Stream 등)는 처음 접하거나 오랜만에 다시 볼 때마다 새롭고 낯설게 느껴질 수 있습니다. 많은 개발자분들이 비슷한 경험을 합니다. 이는 해당 개념들이 다소 저수준이고, 운영체제나 하드웨어와 상호작용하는 방식과 깊이 연관되어 있기 때문일 수 있습니다.

"볼 때는 이해하겠는데 시간이 지나면 다시 낯설어진다"는 것은 개념들이 아직 완전히 내재화되지 않았거나, 각 요소들이 어떻게 유기적으로 연결되어 동작하는지에 대한 큰 그림이 선명하지 않기 때문일 수 있습니다. 본질적인 이해를 돕기 위해 몇 가지 핵심 원리와 비유를 다시 한번 정리해보고, 어떻게 하면 더 깊이 이해할 수 있을지 몇 가지 학습 전략을 제안해 드리고 싶습니다.

**핵심 원리와 비유 다시 보기**

1.  **`Future` (미래의 값): 식당의 주문 번호표**
    *   `Future`는 "언젠가 완료될 작업의 결과"를 나타내는 약속입니다. 당장 결과는 없지만, 나중에 이 번호표를 가지고 오면 음식을 받을 수 있다는 약속과 같습니다.
    *   **본질:** 작업이 완료될 때까지 기다리지 않고 다른 일을 할 수 있게 해주는 추상화입니다.

2.  **`async/await` (편리한 비동기 문법): 요리사에게 맡기고 기다리기**
    *   `async fn`은 `Future`를 반환하는 함수를 쉽게 만들게 해줍니다. 마치 "이 레시피대로 요리해주세요"라고 요리사에게 주문하는 것과 같습니다.
    *   `.await`는 `Future`가 완료될 때까지 현재 함수의 실행을 "논리적으로" 멈추고, 완료되면 그 결과를 가져옵니다. 요리사에게 주문하고 음식이 나올 때까지 잠시 다른 생각을 하다가 음식이 나오면 받는 것과 비슷합니다. (실제로는 스레드가 멈추는 게 아니라 다른 작업을 하러 갑니다.)
    *   **본질:** `Future`를 직접 다루는 복잡함(수동 `poll` 등)을 숨기고, 동기 코드처럼 보이게 만들어 비동기 로직을 더 쉽게 작성하게 해줍니다.

3.  **`poll` 메소드 (상태 점검): "음식 나왔나요?"**
    *   모든 `Future`의 핵심입니다. 실행기(Executor)가 주기적으로 `Future`에게 "작업 끝났니? 결과 줄 수 있니?"라고 물어보는 것입니다.
    *   **`Poll::Ready(value)`**: "네, 여기 결과 나왔습니다!" (음식 준비 완료)
    *   **`Poll::Pending`**: "아니요, 아직 준비 중입니다. 나중에 다시 물어봐 주세요." (음식 아직 조리 중)
    *   **본질:** `Future`의 진행 상태를 외부(실행기)에 알리는 표준화된 방법입니다. 비동기 작업이 협력적으로 멀티태스킹을 할 수 있게 하는 기반입니다.

4.  **`Waker` (깨우기 알림 설정): "음식 다 되면 진동벨 울려주세요"**
    *   `Future`가 `Poll::Pending`을 반환할 때, "나중에 작업이 완료될 가능성이 생기면 이 `Waker`를 통해 알려줘. 그러면 실행기가 다시 나를 `poll`할 거야"라고 등록하는 것입니다. 식당에서 진동벨을 받아두는 것과 같습니다.
    *   **본질:** `Future`가 불필요하게 계속 `poll` 당하는 것을 막고, 실제로 진행이 가능할 때만 `poll` 되도록 하여 효율성을 높입니다.

5.  **`Stream` (비동기적인 값들의 연속): 회전초밥 레일**
    *   `Future`가 단일 값을 비동기적으로 생성한다면, `Stream`은 여러 값을 시간에 걸쳐 비동기적으로 생성합니다. 초밥이 하나씩 레일을 따라 나오는 것과 같습니다.
    *   `poll_next`는 `Future`의 `poll`과 유사하게, 스트림에게 "다음 초밥 나왔나요?"라고 물어보는 것입니다.
    *   **`Poll::Ready(Some(item))`**: "네, 여기 다음 초밥 나왔습니다!"
    *   **`Poll::Ready(None)`**: "레일 위에 더 이상 초밥 없습니다. (스트림 끝)"
    *   **`Poll::Pending`**: "아직 다음 초밥 준비 중입니다."
    *   **본질:** 값의 비동기 시퀀스를 나타내는 추상화입니다.

**본질적 이해를 위한 학습 전략**

1.  **가장 단순한 `Future` 직접 만들어보기:**
    *   `async/await` 없이, `std::future::Future` 트레잇과 `Poll`, `Waker`를 사용해서 아주 간단한 `Future` (예: 특정 시간이 지나면 완료되는 타이머)를 직접 구현해보세요. `Waker`를 어떻게 저장하고 언제 호출해야 하는지 고민하는 과정에서 많은 것을 배울 수 있습니다.
    *   Tokio나 async-std 같은 런타임 없이, 아주 기본적인 실행기를 직접 만들어보는 것도 도움이 됩니다 (물론 실제 사용 목적이 아니라 학습 목적).

2.  **`async/await` 코드의 저수준 변환 상상해보기:**
    *   간단한 `async fn` 코드를 작성한 후, 이 코드가 컴파일러에 의해 대략 어떤 상태 머신과 `poll` 로직으로 변환될지 손으로 그려보거나 상상해보세요. `.await` 지점에서 상태가 어떻게 나뉘고, `Waker`가 어떻게 사용될지 생각해보는 것입니다.

3.  **실행기(Executor)의 역할 이해하기:**
    *   Tokio나 async-std 같은 런타임이 내부적으로 태스크 큐를 어떻게 관리하고, `Waker` 알림을 받으면 어떻게 해당 태스크를 다시 스케줄링하는지 대략적인 그림을 이해하는 것이 중요합니다. 실행기는 이 모든 비동기 조각들을 실제로 움직이게 하는 엔진입니다.

4.  **작은 프로젝트에 적용하며 디버깅하기:**
    *   배운 내용을 바탕으로 간단한 비동기 프로그램을 작성하고, `log`나 `trace`를 사용하여 `poll`이 언제 호출되는지, `Waker`가 언제 깨우는지, 상태가 어떻게 변하는지 등을 직접 눈으로 확인해보세요. 문제가 발생했을 때 왜 그런지 디버깅하는 과정에서 깊은 이해를 얻을 수 있습니다.

5.  **핵심 디자인 철학 고민하기: "차단하지 마라 (Don't Block)"**
    *   Rust 비동기 프로그래밍의 가장 근본적인 원칙 중 하나는 "워커 스레드를 절대 차단하지 마라"입니다. `poll` 함수가 왜 오래 걸리는 동기 작업을 하면 안 되는지, `.await`가 어떻게 스레드 차단 없이 대기를 가능하게 하는지를 계속 생각해보세요. 이 원칙을 이해하면 많은 디자인 결정들이 왜 그렇게 이루어졌는지 알 수 있습니다.

6.  **문서와 예제 반복 학습 및 질문:**
    *   공식 문서(Rust 비동기 책, Tokio 튜토리얼 등)와 다양한 예제를 꾸준히 보는 것이 중요합니다. 한 번 봐서는 와닿지 않던 내용도 여러 번 보거나, 다른 예제와 비교해보면 연결고리가 보일 때가 있습니다. 그리고 지금처럼 궁금한 점을 계속 질문하는 것도 아주 좋은 학습 방법입니다!

**시간이 지나면 낯설어지는 것은 자연스러운 현상입니다.** 중요한 것은 다시 봤을 때 이전보다 조금 더 빨리, 조금 더 깊이 이해할 수 있게 되는 것입니다. 반복적인 학습과 실제 코드 작성 경험을 통해 조금씩 익숙해지고 내재화될 것이라고 생각합니다. 조급해하지 마시고 꾸준히 탐구해보세요!

혹시 특정 부분에 대해 더 자세한 설명이나 예시가 필요하시면 언제든지 말씀해주세요.
